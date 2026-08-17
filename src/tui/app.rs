//! Application state and its pure transitions: ticking the sprite and
//! status-word rotation, handling key input, and applying `TuiEvent`s
//! from the orchestrator. Nothing here touches a terminal, a provider,
//! or the filesystem — that separation is what makes it testable
//! without a real terminal (see this module's tests) and is required by
//! `tui`'s module boundary ("render loop only; never makes
//! network/provider calls itself").

use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio_util::sync::CancellationToken;
use tui_textarea::TextArea;

use crate::stage::Rigor;
use crate::tui::sprite::{self, SpriteState};
use crate::tui::status_words;

/// How long a first `ctrl-c` stays "armed" waiting for the second press
/// before the hint disappears and a third press would start over.
const CTRL_C_ARM_WINDOW: Duration = Duration::from_secs(2);

/// Events the orchestrator (`cli.rs`'s background thread driving the
/// actual provider/agent call) sends into the TUI over an
/// `std::sync::mpsc::channel`. Deliberately UI-facing and decoupled from
/// `provider::Delta`/`tools::ToolCall` — `tui` never imports those types.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// Not sent by `cli.rs` today — `dlt tui run`/`dlt tui build` each
    /// drive exactly one stage/operation per session, so `App::new`
    /// already carries the initial stage/rigor and there's never a
    /// mid-session transition to announce. Kept (and tested via
    /// `on_event`, see `started_event_names_the_stage_and_resets_
    /// status_rotation`) for a future multi-stage TUI session that
    /// walks proposal → design → tasks without exiting between them.
    #[allow(dead_code)]
    Started {
        stage: String,
        rigor: Rigor,
    },
    Text(String),
    Reasoning(String),
    ToolCall {
        tool: String,
        input: String,
    },
    /// No `tool` field: `AgentObserver::on_tool_result`'s signature only
    /// carries the outcome, not the call that produced it (the
    /// preceding `ToolCall` transcript entry already named it).
    ToolResult {
        success: bool,
        output: String,
    },
    TokenCount(u32),
    /// Prompt-assembly info for the Info pane — mirrors
    /// `stage::context::Assembled`'s `dropped`/`token_count` without
    /// `tui` depending on that type directly.
    PromptInfo {
        dropped: Vec<String>,
        context_window: u32,
    },
    /// A `Prompt`-gated tool call is waiting on a yes/no answer. Sent by
    /// `cli.rs`'s `TuiApprover` (which blocks on a response channel
    /// rather than reading stdin directly — reading stdin from the
    /// background thread while the TUI's own render loop polls the same
    /// fd for key events on the main thread would race two consumers
    /// against one input stream). Answered via `App::answer_approval`.
    ApprovalRequest {
        tool: String,
        preview: String,
    },
    Done,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum TranscriptEntry {
    Text(String),
    Reasoning(String),
    ToolCall { tool: String, input: String },
    ToolResult { success: bool, summary: String },
    System(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Transcript,
    Info,
}

impl Pane {
    fn next(self) -> Self {
        match self {
            Pane::Transcript => Pane::Info,
            Pane::Info => Pane::Transcript,
        }
    }
}

pub struct App {
    pub slug: String,
    pub stage: String,
    pub rigor: Rigor,
    pub started_at: Instant,
    pub transcript: Vec<TranscriptEntry>,
    pub token_count: u32,
    pub context_window: u32,
    pub dropped_context: Vec<String>,
    pub sprite_state: SpriteState,
    pub sprite_frame: usize,
    last_sprite_tick: Instant,
    pub status_word_index: usize,
    last_status_tick: Instant,
    pub active_pane: Pane,
    pub command_bar: Option<TextArea<'static>>,
    pub show_help: bool,
    ctrl_c_armed_at: Option<Instant>,
    pub reasoning_collapsed: bool,
    pub done: bool,
    /// `true` only if the user interrupted a still-in-flight run (`esc`,
    /// or quitting before `Done`/`Error` arrived) — distinct from
    /// `done`, which is also `true` after a normal successful finish.
    /// This is what `cli.rs` checks to decide whether to write the
    /// artifact after the TUI closes.
    pub cancelled: bool,
    pub error: Option<String>,
    pub should_quit: bool,
    pub dirty: bool,
    cancel: CancellationToken,
    /// `Some((tool, preview))` while a `Prompt`-gated tool call is
    /// waiting on an answer. `None` for `dlt tui run`, which never
    /// prompts for approval.
    pub approval_pending: Option<(String, String)>,
    answer_tx: Option<mpsc::Sender<bool>>,
}

impl App {
    pub fn new(
        slug: String,
        stage: String,
        rigor: Rigor,
        cancel: CancellationToken,
        answer_tx: Option<mpsc::Sender<bool>>,
    ) -> Self {
        let now = Instant::now();
        App {
            slug,
            stage,
            rigor,
            started_at: now,
            transcript: Vec::new(),
            token_count: 0,
            context_window: 0,
            dropped_context: Vec::new(),
            // Waiting for the first token — genuinely idle until the
            // provider/tool loop actually produces something.
            sprite_state: SpriteState::Idle,
            sprite_frame: 0,
            last_sprite_tick: now,
            status_word_index: 0,
            last_status_tick: now,
            active_pane: Pane::Transcript,
            command_bar: None,
            show_help: false,
            ctrl_c_armed_at: None,
            reasoning_collapsed: true,
            done: false,
            cancelled: false,
            error: None,
            should_quit: false,
            dirty: true,
            cancel,
            approval_pending: None,
            answer_tx,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn status_word(&self) -> &'static str {
        let words = status_words::words_for_stage(&self.stage);
        words[self.status_word_index % words.len()]
    }

    pub fn sprite_glyph(&self) -> &'static str {
        sprite::frame(self.sprite_state, self.sprite_frame)
    }

    /// Advance animation state for the time elapsed since the last
    /// tick. Called on a fixed interval by the render loop — cheap and
    /// idempotent-ish (only sets `dirty` when something actually
    /// changed), so calling it more often than strictly necessary never
    /// causes visible harm, only a wasted redraw check.
    pub fn on_tick(&mut self, now: Instant) {
        if now.duration_since(self.last_sprite_tick) >= sprite::FRAME_INTERVAL {
            self.sprite_frame = self.sprite_frame.wrapping_add(1);
            self.last_sprite_tick = now;
            self.dirty = true;
        }
        if !self.done && now.duration_since(self.last_status_tick) >= status_words::ROTATE_INTERVAL
        {
            self.status_word_index = self.status_word_index.wrapping_add(1);
            self.last_status_tick = now;
            self.dirty = true;
        }
        if let Some(armed_at) = self.ctrl_c_armed_at
            && now.duration_since(armed_at) >= CTRL_C_ARM_WINDOW
        {
            self.ctrl_c_armed_at = None;
            self.dirty = true;
        }
        // The footer's elapsed-time clock needs a redraw at least once a
        // second even when nothing else changed.
        self.dirty = true;
    }

    pub fn on_event(&mut self, event: TuiEvent) {
        self.dirty = true;
        // The first sign of actual activity ends the "idle, waiting for
        // the provider/tool loop to produce anything" state.
        if self.sprite_state == SpriteState::Idle
            && matches!(
                event,
                TuiEvent::Text(_)
                    | TuiEvent::Reasoning(_)
                    | TuiEvent::ToolCall { .. }
                    | TuiEvent::ToolResult { .. }
            )
        {
            self.sprite_state = SpriteState::Working;
        }
        match event {
            TuiEvent::Started { stage, rigor } => {
                self.stage = stage.clone();
                self.rigor = rigor;
                self.status_word_index = 0;
                self.last_status_tick = Instant::now();
                self.transcript
                    .push(TranscriptEntry::System(format!("Started stage '{stage}'")));
            }
            TuiEvent::Text(text) => self.transcript.push(TranscriptEntry::Text(text)),
            TuiEvent::Reasoning(text) => self.transcript.push(TranscriptEntry::Reasoning(text)),
            TuiEvent::ToolCall { tool, input } => {
                self.transcript
                    .push(TranscriptEntry::ToolCall { tool, input });
            }
            TuiEvent::ToolResult { success, output } => {
                self.transcript.push(TranscriptEntry::ToolResult {
                    success,
                    summary: output,
                });
            }
            TuiEvent::TokenCount(count) => self.token_count = count,
            TuiEvent::PromptInfo {
                dropped,
                context_window,
            } => {
                self.dropped_context = dropped;
                self.context_window = context_window;
            }
            TuiEvent::ApprovalRequest { tool, preview } => {
                self.approval_pending = Some((tool, preview));
            }
            TuiEvent::Done => {
                self.done = true;
                self.sprite_state = SpriteState::Done;
            }
            TuiEvent::Error(message) => {
                self.done = true;
                self.sprite_state = SpriteState::Done;
                self.error = Some(message.clone());
                self.transcript.push(TranscriptEntry::Error(message));
            }
        }
    }

    /// `true` if the key was consumed as a command-bar keystroke rather
    /// than a global shortcut — callers don't need to know the
    /// difference, but it's what makes this function's control flow
    /// legible.
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        self.dirty = true;

        // A pending approval takes strict priority over everything else
        // — the background thread is blocked waiting on it, so there's
        // nothing else meaningful for a keystroke to do right now.
        if self.approval_pending.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.answer_approval(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.answer_approval(false);
                }
                _ => {}
            }
            return;
        }

        if let Some(bar) = self.command_bar.as_mut() {
            match key.code {
                KeyCode::Esc => self.command_bar = None,
                KeyCode::Enter => {
                    let command = bar.lines().join("\n");
                    self.command_bar = None;
                    self.run_command(command.trim());
                }
                _ => {
                    bar.input(ratatui::crossterm::event::Event::Key(key));
                }
            }
            return;
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.handle_ctrl_c();
            return;
        }

        match key.code {
            KeyCode::Esc => {
                if !self.done {
                    self.cancel.cancel();
                    self.transcript
                        .push(TranscriptEntry::System("Interrupted.".to_string()));
                    self.done = true;
                    self.cancelled = true;
                    self.sprite_state = SpriteState::Done;
                }
            }
            KeyCode::Tab => self.active_pane = self.active_pane.next(),
            KeyCode::Char('/') => self.command_bar = Some(TextArea::default()),
            KeyCode::Char('?') => self.show_help = !self.show_help,
            _ => {}
        }
    }

    /// Answer the pending approval (if any) and clear it. Sends `false`
    /// if there's no way to deliver the answer (no `answer_tx`, or the
    /// background thread's `TuiApprover` already stopped listening) so
    /// the tool defaults to denied rather than the background thread
    /// hanging forever.
    fn answer_approval(&mut self, approved: bool) {
        if self.approval_pending.take().is_some()
            && let Some(tx) = &self.answer_tx
        {
            let _ = tx.send(approved);
        }
    }

    fn handle_ctrl_c(&mut self) {
        let now = Instant::now();
        match self.ctrl_c_armed_at {
            Some(armed_at) if now.duration_since(armed_at) < CTRL_C_ARM_WINDOW => {
                self.quit_now();
            }
            _ => self.ctrl_c_armed_at = Some(now),
        }
    }

    /// Quit immediately. If the run was still in flight, this counts as
    /// a cancellation (same as `esc`) rather than a normal finish.
    fn quit_now(&mut self) {
        self.cancel.cancel();
        if !self.done {
            self.done = true;
            self.cancelled = true;
            self.sprite_state = SpriteState::Done;
        }
        self.should_quit = true;
    }

    fn run_command(&mut self, command: &str) {
        match command {
            "reasoning" => self.reasoning_collapsed = !self.reasoning_collapsed,
            "help" => self.show_help = !self.show_help,
            "quit" => self.quit_now(),
            "" => {}
            other => self
                .transcript
                .push(TranscriptEntry::System(format!("unknown command: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(
            "my-change".to_string(),
            "proposal".to_string(),
            Rigor::Standard,
            CancellationToken::new(),
            None,
        )
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn tick_advances_sprite_frame_after_the_interval() {
        let mut app = app();
        let start_frame = app.sprite_frame;
        app.on_tick(Instant::now() + sprite::FRAME_INTERVAL);
        assert_ne!(app.sprite_frame, start_frame);
    }

    #[test]
    fn tick_before_the_interval_does_not_advance_the_frame() {
        let mut app = app();
        let start_frame = app.sprite_frame;
        app.on_tick(Instant::now());
        assert_eq!(app.sprite_frame, start_frame);
    }

    #[test]
    fn status_word_rotates_after_its_interval() {
        let mut app = app();
        let start = app.status_word_index;
        app.on_tick(Instant::now() + status_words::ROTATE_INTERVAL);
        assert_ne!(app.status_word_index, start);
    }

    #[test]
    fn tab_cycles_between_the_two_panes() {
        let mut app = app();
        assert_eq!(app.active_pane, Pane::Transcript);
        app.on_key(press(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Info);
        app.on_key(press(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Transcript);
    }

    #[test]
    fn esc_cancels_and_marks_done() {
        let cancel = CancellationToken::new();
        let mut app = App::new(
            "s".to_string(),
            "proposal".to_string(),
            Rigor::Standard,
            cancel.clone(),
            None,
        );
        app.on_key(press(KeyCode::Esc));
        assert!(cancel.is_cancelled());
        assert!(app.done);
        assert!(app.cancelled, "esc on a live run must count as cancelled");
    }

    #[test]
    fn quitting_before_completion_counts_as_cancelled() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.cancelled, "quitting mid-run must count as cancelled");
    }

    #[test]
    fn quitting_after_normal_completion_is_not_cancelled() {
        let mut app = app();
        app.on_event(TuiEvent::Done);
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        assert!(
            !app.cancelled,
            "quitting the viewer after a normal finish must not retroactively count as cancelled"
        );
    }

    #[test]
    fn esc_after_already_done_does_not_reinterrupt() {
        let mut app = app();
        app.on_event(TuiEvent::Done);
        let len_before = app.transcript.len();
        app.on_key(press(KeyCode::Esc));
        assert_eq!(app.transcript.len(), len_before);
    }

    #[test]
    fn single_ctrl_c_arms_but_does_not_quit() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.should_quit);
    }

    #[test]
    fn two_ctrl_c_within_the_window_quits() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn ctrl_c_arm_expires_after_the_window() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.on_tick(Instant::now() + CTRL_C_ARM_WINDOW);
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(
            !app.should_quit,
            "arm should have expired, not carried over"
        );
    }

    #[test]
    fn slash_opens_command_bar_and_esc_closes_it_without_effect() {
        let mut app = app();
        app.on_key(press(KeyCode::Char('/')));
        assert!(app.command_bar.is_some());
        app.on_key(press(KeyCode::Esc));
        assert!(app.command_bar.is_none());
        assert!(!app.done, "esc inside the command bar must not interrupt");
    }

    #[test]
    fn question_mark_toggles_help() {
        let mut app = app();
        assert!(!app.show_help);
        app.on_key(press(KeyCode::Char('?')));
        assert!(app.show_help);
        app.on_key(press(KeyCode::Char('?')));
        assert!(!app.show_help);
    }

    #[test]
    fn reasoning_command_toggles_collapse_state() {
        let mut app = app();
        let before = app.reasoning_collapsed;
        app.run_command("reasoning");
        assert_eq!(app.reasoning_collapsed, !before);
    }

    #[test]
    fn quit_command_cancels_and_quits() {
        let cancel = CancellationToken::new();
        let mut app = App::new(
            "s".to_string(),
            "proposal".to_string(),
            Rigor::Standard,
            cancel.clone(),
            None,
        );
        app.run_command("quit");
        assert!(app.should_quit);
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn unknown_command_is_reported_in_the_transcript() {
        let mut app = app();
        app.run_command("bogus");
        assert!(matches!(
            app.transcript.last(),
            Some(TranscriptEntry::System(msg)) if msg.contains("bogus")
        ));
    }

    #[test]
    fn text_and_reasoning_events_land_in_the_transcript_distinctly() {
        let mut app = app();
        app.on_event(TuiEvent::Text("hello".to_string()));
        app.on_event(TuiEvent::Reasoning("thinking".to_string()));
        assert!(matches!(app.transcript[0], TranscriptEntry::Text(ref t) if t == "hello"));
        assert!(matches!(app.transcript[1], TranscriptEntry::Reasoning(ref t) if t == "thinking"));
    }

    #[test]
    fn sprite_starts_idle_and_moves_to_working_on_first_activity() {
        let mut app = app();
        assert_eq!(app.sprite_state, SpriteState::Idle);
        app.on_event(TuiEvent::Text("hello".to_string()));
        assert_eq!(app.sprite_state, SpriteState::Working);
    }

    #[test]
    fn started_event_names_the_stage_and_resets_status_rotation() {
        let mut app = app();
        app.status_word_index = 3;
        app.on_event(TuiEvent::Started {
            stage: "design".to_string(),
            rigor: Rigor::Deep,
        });
        assert_eq!(app.stage, "design");
        assert_eq!(app.rigor, Rigor::Deep);
        assert_eq!(app.status_word_index, 0);
        assert!(matches!(
            app.transcript.last(),
            Some(TranscriptEntry::System(msg)) if msg.contains("design")
        ));
    }

    #[test]
    fn done_event_stops_the_status_word_from_rotating() {
        let mut app = app();
        app.on_event(TuiEvent::Done);
        let start = app.status_word_index;
        app.on_tick(Instant::now() + status_words::ROTATE_INTERVAL * 2);
        assert_eq!(app.status_word_index, start);
    }

    #[test]
    fn error_event_marks_done_and_records_the_message() {
        let mut app = app();
        app.on_event(TuiEvent::Error("boom".to_string()));
        assert!(app.done);
        assert_eq!(app.error.as_deref(), Some("boom"));
        assert!(matches!(
            app.transcript.last(),
            Some(TranscriptEntry::Error(_))
        ));
    }

    #[test]
    fn approval_request_sets_pending_state() {
        let mut app = app();
        app.on_event(TuiEvent::ApprovalRequest {
            tool: "write_file".to_string(),
            preview: "+ hello".to_string(),
        });
        assert_eq!(
            app.approval_pending,
            Some(("write_file".to_string(), "+ hello".to_string()))
        );
    }

    #[test]
    fn pressing_y_answers_true_and_clears_pending() {
        let (tx, rx) = mpsc::channel();
        let mut app = App::new(
            "s".to_string(),
            "build".to_string(),
            Rigor::Standard,
            CancellationToken::new(),
            Some(tx),
        );
        app.on_event(TuiEvent::ApprovalRequest {
            tool: "write_file".to_string(),
            preview: "+ hello".to_string(),
        });
        app.on_key(press(KeyCode::Char('y')));
        assert!(app.approval_pending.is_none());
        assert_eq!(rx.try_recv(), Ok(true));
    }

    #[test]
    fn pressing_n_or_esc_answers_false_and_clears_pending() {
        let (tx, rx) = mpsc::channel();
        let mut app = App::new(
            "s".to_string(),
            "build".to_string(),
            Rigor::Standard,
            CancellationToken::new(),
            Some(tx),
        );
        app.on_event(TuiEvent::ApprovalRequest {
            tool: "run_command".to_string(),
            preview: "cargo test".to_string(),
        });
        app.on_key(press(KeyCode::Char('n')));
        assert!(app.approval_pending.is_none());
        assert_eq!(rx.try_recv(), Ok(false));
    }

    #[test]
    fn approval_pending_takes_priority_over_other_keys() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(
            "s".to_string(),
            "build".to_string(),
            Rigor::Standard,
            CancellationToken::new(),
            Some(tx),
        );
        app.on_event(TuiEvent::ApprovalRequest {
            tool: "write_file".to_string(),
            preview: "+ hello".to_string(),
        });
        // esc must answer "no", not interrupt the run, while a
        // decision is pending.
        app.on_key(press(KeyCode::Esc));
        assert!(
            !app.done,
            "esc while an approval is pending must not interrupt the run"
        );
    }
}
