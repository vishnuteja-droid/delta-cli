//! Terminal UI: the render loop, panes, and status display. Owns the
//! terminal and ticks on a fixed interval independent of network state
//! — the non-negotiable threading rule from `PLAN.md`. Provider deltas
//! and tool events arrive pre-translated into [`app::TuiEvent`]s over an
//! `std::sync::mpsc` channel from a background thread `cli.rs` spawns
//! with its own tokio runtime; this module never imports `Provider` or
//! `tools`, never calls either, and never blocks on either — the render
//! loop only ever does a non-blocking channel drain, a non-blocking
//! input poll, and a draw.

pub mod app;
pub mod color;
pub mod render;
pub mod sprite;
pub mod status_words;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event};

use app::{App, TuiEvent};
use sprite::SpriteState;

use crate::error::TuiError;

/// One iteration of the loop drains at most this long between input
/// polls and ticks — small enough that the sprite's 10fps animation and
/// the footer's elapsed-time clock both stay visually smooth.
const TICK_INTERVAL: Duration = Duration::from_millis(100);

pub struct TuiOutcome {
    /// `true` only if the run finished normally: a `TuiEvent::Done`
    /// arrived and the user never interrupted or quit early. `cli.rs`
    /// uses this to decide whether to write the stage artifact.
    pub completed: bool,
    pub error: Option<String>,
}

/// Own the terminal for the lifetime of one TUI session: initialize,
/// run the event loop until the user quits or the operation finishes
/// and they close the viewer, then restore the terminal unconditionally
/// — including on an error return, so a failure here never leaves the
/// caller's shell in raw mode or the alternate screen.
pub fn run(mut app: App, events: mpsc::Receiver<TuiEvent>) -> Result<TuiOutcome, TuiError> {
    let mut terminal = ratatui::try_init().map_err(TuiError::Io)?;
    let result = run_loop(&mut terminal, &mut app, &events);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    events: &mpsc::Receiver<TuiEvent>,
) -> Result<TuiOutcome, TuiError> {
    loop {
        drain_events(app, events);
        poll_input(app)?;
        app.on_tick(Instant::now());

        if app.dirty {
            terminal
                .draw(|frame| render::draw(frame, app))
                .map_err(TuiError::Io)?;
            app.dirty = false;
        }

        if app.should_quit {
            break;
        }

        std::thread::sleep(TICK_INTERVAL);
    }

    Ok(TuiOutcome {
        completed: app.done && !app.cancelled && app.error.is_none(),
        error: app.error.clone(),
    })
}

/// Drain every event already waiting without blocking — the render
/// loop must never wait on the channel, only check it. A disconnected
/// sender (the orchestrator thread ended without a final `Done`/`Error`,
/// e.g. it panicked) is treated as an implicit finish so the UI doesn't
/// spin forever waiting for a message that will never arrive.
fn drain_events(app: &mut App, events: &mpsc::Receiver<TuiEvent>) {
    loop {
        match events.try_recv() {
            Ok(event) => app.on_event(event),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                if !app.done {
                    app.done = true;
                    app.sprite_state = SpriteState::Done;
                    app.dirty = true;
                }
                return;
            }
        }
    }
}

/// Non-blocking input poll: a zero-duration `event::poll` check, so
/// this never stalls the tick/redraw cadence waiting for a keypress.
fn poll_input(app: &mut App) -> Result<(), TuiError> {
    if !event::poll(Duration::ZERO).map_err(TuiError::Io)? {
        return Ok(());
    }
    match event::read().map_err(TuiError::Io)? {
        Event::Key(key) => app.on_key(key),
        Event::Resize(_, _) => app.dirty = true,
        _ => {}
    }
    Ok(())
}
