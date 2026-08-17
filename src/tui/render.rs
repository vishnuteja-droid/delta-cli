//! Pure rendering: `draw(frame, app)` builds the whole UI from `App`
//! state alone — no I/O, no provider/network calls, honoring the same
//! module boundary as the rest of `tui`. Testable headlessly via
//! `ratatui::backend::TestBackend` (see this module's tests) rather
//! than a real terminal.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::tui::app::{App, Pane, TranscriptEntry};
use crate::tui::color;

pub fn draw(frame: &mut Frame, app: &App) {
    let truecolor = color::supports_truecolor();
    let primary = color::accent_primary(truecolor);
    let secondary = color::accent_secondary(truecolor);

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, chunks[0], app, primary);
    draw_body(frame, chunks[1], app, primary, secondary);
    draw_footer(frame, chunks[2], app, primary);

    if app.command_bar.is_some() {
        draw_command_bar(frame, area, app, primary);
    }
    if app.show_help {
        draw_help(frame, area, primary);
    }
    if let Some((tool, preview)) = &app.approval_pending {
        draw_approval(frame, area, tool, preview, secondary);
    }
}

/// "Header with change slug + stage + rigor," plus the sprite and the
/// current stage-keyed status word.
fn draw_header(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(format!(" dlt · {} ", app.slug));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let line = Line::from(vec![
        Span::styled(
            format!("{} ", app.sprite_glyph()),
            Style::default().fg(accent),
        ),
        Span::raw(format!("stage: {}  ", app.stage)),
        Span::raw(format!("rigor: {}  ", app.rigor)),
        Span::styled(
            app.status_word(),
            Style::default().fg(accent).add_modifier(Modifier::ITALIC),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App, primary: Color, secondary: Color) {
    match app.active_pane {
        Pane::Transcript => draw_transcript(frame, area, app, primary),
        Pane::Info => draw_info(frame, area, app, secondary),
    }
}

/// The main transcript pane: text, collapsible dim-italic reasoning
/// blocks, and tool call/result lines, tail-fit to the pane's height so
/// the most recent content is always what's visible (no scroll key is
/// in `PLAN.md`'s key list, so this always shows the tail rather than
/// supporting manual scrolling).
fn draw_transcript(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let border_style = if app.active_pane == Pane::Transcript {
        Style::default().fg(accent)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Transcript (tab: info) ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = transcript_lines(app, inner.width);
    let visible = inner.height as usize;
    if lines.len() > visible {
        lines = lines.split_off(lines.len() - visible);
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn transcript_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    for entry in &app.transcript {
        match entry {
            TranscriptEntry::Text(text) => {
                for wrapped in textwrap::wrap(text, width) {
                    lines.push(Line::from(wrapped.into_owned()));
                }
            }
            TranscriptEntry::Reasoning(text) => {
                push_reasoning_lines(&mut lines, text, width, app.reasoning_collapsed);
            }
            TranscriptEntry::ToolCall { tool, input } => {
                for wrapped in textwrap::wrap(&format!("▶ {tool} {input}"), width) {
                    lines.push(Line::from(Span::styled(
                        wrapped.into_owned(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )));
                }
            }
            TranscriptEntry::ToolResult { success, summary } => {
                let marker = if *success { "✓" } else { "✗" };
                for wrapped in textwrap::wrap(&format!("{marker} {summary}"), width) {
                    lines.push(Line::from(wrapped.into_owned()));
                }
            }
            TranscriptEntry::System(text) => {
                for wrapped in textwrap::wrap(text, width) {
                    lines.push(Line::from(Span::styled(
                        wrapped.into_owned(),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                }
            }
            TranscriptEntry::Error(text) => {
                for wrapped in textwrap::wrap(&format!("error: {text}"), width) {
                    lines.push(Line::from(Span::styled(
                        wrapped.into_owned(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )));
                }
            }
        }
    }
    lines
}

fn push_reasoning_lines(lines: &mut Vec<Line<'static>>, text: &str, width: usize, collapsed: bool) {
    let style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);
    if collapsed {
        let chars = text.chars().count();
        lines.push(Line::from(Span::styled(
            format!("· reasoning ({chars} chars) — /reasoning to expand"),
            style,
        )));
        return;
    }
    for wrapped in textwrap::wrap(text, width.saturating_sub(2).max(1)) {
        lines.push(Line::from(Span::styled(format!("  {wrapped}"), style)));
    }
}

/// The inspector pane: token budget, what got dropped from context, and
/// the last error if any — the prompt-assembly detail `--dry-run`
/// prints on the plain CLI path, kept visible while the TUI runs.
fn draw_info(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let border_style = if app.active_pane == Pane::Info {
        Style::default().fg(accent)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Info (tab: transcript) ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(format!("change: {}", app.slug)),
        Line::from(format!("stage: {}", app.stage)),
        Line::from(format!("rigor: {}", app.rigor)),
        Line::from(format!("token budget: {}", app.context_window)),
        Line::from(format!("tokens used: {}", app.token_count)),
    ];
    if app.dropped_context.is_empty() {
        lines.push(Line::from("dropped from context: none"));
    } else {
        lines.push(Line::from(format!(
            "dropped from context: {}",
            app.dropped_context.join(", ")
        )));
    }
    if let Some(err) = &app.error {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default().add_modifier(Modifier::BOLD),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// "Sticky footer with elapsed time, token count, and esc to interrupt."
fn draw_footer(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let elapsed = app.elapsed();
    let minutes = elapsed.as_secs() / 60;
    let seconds = elapsed.as_secs() % 60;
    let hint = if app.done {
        "done — ctrl-c twice to quit"
    } else {
        "esc to interrupt · ctrl-c twice to quit · tab panes · / commands · ? help"
    };
    let line = Line::from(vec![
        Span::styled(
            format!(" {minutes:02}:{seconds:02} "),
            Style::default().fg(accent),
        ),
        Span::raw(format!("· {} tokens · ", app.token_count)),
        Span::styled(hint, Style::default().add_modifier(Modifier::DIM)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_command_bar(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let Some(bar) = app.command_bar.as_ref() else {
        return;
    };
    let bar_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(4),
        width: area.width,
        height: 3,
    };
    frame.render_widget(Clear, bar_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(" command (enter: run, esc: cancel) ");
    let inner = block.inner(bar_area);
    frame.render_widget(block, bar_area);
    frame.render_widget(bar, inner);
}

fn draw_help(frame: &mut Frame, area: Rect, accent: Color) {
    let width = area.width.saturating_sub(10).clamp(20, 50);
    let height = 9;
    let help_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, help_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(" help (? to close) ");
    let inner = block.inner(help_area);
    frame.render_widget(block, help_area);

    let lines = vec![
        Line::from("esc      interrupt the current run"),
        Line::from("ctrl-c×2 quit"),
        Line::from("tab      cycle panes"),
        Line::from("/        command bar (reasoning, help, quit)"),
        Line::from("?        toggle this help"),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// A `Prompt`-gated tool call's diff/command, with a yes/no prompt —
/// "print the diff or the exact command before asking," per `PLAN.md`.
fn draw_approval(frame: &mut Frame, area: Rect, tool: &str, preview: &str, accent: Color) {
    let width = area.width.saturating_sub(6).clamp(20, 76);
    let preview_lines = textwrap::wrap(preview, usize::from(width.saturating_sub(2)));
    let height = (preview_lines.len() as u16 + 4).min(area.height.saturating_sub(2));
    let approval_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, approval_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(format!(" approve '{tool}'? (y/n) "));
    let inner = block.inner(approval_area);
    frame.render_widget(block, approval_area);

    let mut lines: Vec<Line> = preview_lines
        .into_iter()
        .map(|line| Line::from(line.into_owned()))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[y] approve   [n] deny",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::Rigor;
    use crate::tui::app::{App, TuiEvent};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio_util::sync::CancellationToken;

    fn rendered(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().to_string()
    }

    fn app() -> App {
        App::new(
            "my-feature".to_string(),
            "proposal".to_string(),
            Rigor::Standard,
            CancellationToken::new(),
            None,
        )
    }

    #[test]
    fn header_shows_slug_stage_and_rigor() {
        let screen = rendered(&app());
        assert!(screen.contains("my-feature"), "screen: {screen}");
        assert!(screen.contains("stage: proposal"), "screen: {screen}");
        assert!(screen.contains("rigor: standard"), "screen: {screen}");
    }

    #[test]
    fn footer_shows_elapsed_and_esc_hint() {
        let screen = rendered(&app());
        assert!(screen.contains("esc to interrupt"), "screen: {screen}");
        assert!(screen.contains("00:00"), "screen: {screen}");
    }

    #[test]
    fn transcript_text_is_visible() {
        let mut app = app();
        app.on_event(TuiEvent::Text("hello from the model".to_string()));
        let screen = rendered(&app);
        assert!(screen.contains("hello from the model"), "screen: {screen}");
    }

    #[test]
    fn collapsed_reasoning_shows_a_summary_not_the_full_text() {
        let mut app = app();
        app.on_event(TuiEvent::Reasoning(
            "a very long chain of thought".to_string(),
        ));
        let screen = rendered(&app);
        assert!(screen.contains("reasoning ("), "screen: {screen}");
        assert!(
            !screen.contains("a very long chain of thought"),
            "screen: {screen}"
        );
    }

    #[test]
    fn expanded_reasoning_shows_the_full_text() {
        let mut app = app();
        app.on_event(TuiEvent::Reasoning("visible thoughts".to_string()));
        app.reasoning_collapsed = false;
        let screen = rendered(&app);
        assert!(screen.contains("visible thoughts"), "screen: {screen}");
    }

    #[test]
    fn tab_switches_to_the_info_pane_which_shows_the_context_window() {
        let mut app = app();
        app.on_event(TuiEvent::PromptInfo {
            dropped: vec!["repo_tree".to_string()],
            context_window: 100_000,
        });
        app.active_pane = Pane::Info;
        let screen = rendered(&app);
        assert!(screen.contains("token budget: 100000"), "screen: {screen}");
        assert!(screen.contains("repo_tree"), "screen: {screen}");
    }

    #[test]
    fn command_bar_renders_when_open() {
        let mut app = app();
        app.on_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('/'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        ));
        let screen = rendered(&app);
        assert!(screen.contains("command"), "screen: {screen}");
    }

    #[test]
    fn help_overlay_lists_every_key() {
        let mut app = app();
        app.show_help = true;
        let screen = rendered(&app);
        for key_hint in ["esc", "ctrl-c", "tab", "command bar", "help"] {
            assert!(
                screen.contains(key_hint),
                "missing {key_hint:?} in: {screen}"
            );
        }
    }

    #[test]
    fn approval_overlay_shows_the_tool_preview_and_prompt() {
        let mut app = app();
        app.on_event(TuiEvent::ApprovalRequest {
            tool: "write_file".to_string(),
            preview: "+ new line of content".to_string(),
        });
        let screen = rendered(&app);
        assert!(screen.contains("write_file"), "screen: {screen}");
        assert!(screen.contains("new line of content"), "screen: {screen}");
        assert!(screen.contains("approve"), "screen: {screen}");
    }
}
