mod command;
mod header;
mod log_panel;
mod position;
mod satellites;
mod skyplot;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use dx_gp21::GnssSession;

use crate::app::App;

pub fn render<S: GnssSession>(frame: &mut Frame, app: &mut App<S>) {
    let area = frame.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(16),
            Constraint::Min(6),
            Constraint::Length(7),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);

    header::render(frame, rows[0], app);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(26),
            Constraint::Fill(1),
            Constraint::Length(38),
        ])
        .split(rows[1]);

    position::render(frame, top_cols[0], app);
    skyplot::render(frame, top_cols[1], app);
    position::render_right(frame, top_cols[2], app);

    satellites::render(frame, rows[2], app);
    command::render(frame, rows[3], app);
    log_panel::render(frame, rows[4], app);
    render_hint_bar(frame, rows[5], app);

    if app.show_help { render_help(frame, area); }
    if let Some(mode) = app.confirm_restart { render_confirm(frame, area, mode); }
}

fn render_hint_bar<S: GnssSession>(frame: &mut Frame, area: ratatui::layout::Rect, app: &App<S>) {
    // Key names: bright cyan on dark background — clearly readable
    // Descriptions: plain white (not bold) — legible but subordinate to the key
    let sep  = Style::default().fg(Color::DarkGray);
    let key  = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let desc = Style::default().fg(Color::White);

    // Helper: one key+description pair followed by a separator gap
    macro_rules! hint {
        ($k:expr, $d:expr) => {
            vec![
                Span::styled($k, key),
                Span::styled(concat!(" ", $d), desc),
                Span::styled("  ", sep),
            ]
        };
    }

    let spans: Vec<Span> = if app.session.is_readonly() {
        let mut v = vec![
            Span::styled(" File playback mode", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("  │  ", sep),
        ];
        v.extend(hint!("Ctrl+C", "quit"));
        v.extend(hint!("F1", "help"));
        v.extend(hint!("F2/F5", "pause log"));
        v.extend(hint!("F6", "clear log"));
        v.extend(hint!("PgUp/PgDn", "scroll sats"));
        v
    } else {
        let mut v = Vec::new();
        v.extend(hint!("Ctrl+C", "quit"));
        v.extend(hint!("Tab", "autocomplete"));
        v.extend(hint!("↑↓", "history"));
        v.extend(hint!("Enter", "send"));
        v.extend(hint!("F1", "help"));
        v.extend(hint!("F3", "save config"));
        v.extend(hint!("F4", "restart"));
        v.extend(hint!("F2/F5", "pause log"));
        v.extend(hint!("PgUp/PgDn", "scroll sats"));
        v
    };

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black)),
        area,
    );
}

fn render_help(frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    use ratatui::layout::Rect;

    let w = 62u16.min(area.width.saturating_sub(4));
    let h = 20u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x: area.x + x, y: area.y + y, width: w, height: h };

    frame.render_widget(Clear, popup);
    let text = concat!(
        " KEYBOARD SHORTCUTS\n\n",
        " Ctrl+C       Quit\n",
        " Tab          Cycle autocomplete suggestions\n",
        " Shift+Tab    Cycle backwards\n",
        " ↑ / ↓        Navigate command history\n",
        " Enter        Send command\n",
        " Esc          Clear input / dismiss overlay\n",
        " PgUp / PgDn  Scroll satellite table\n\n",
        " F1           Toggle this help overlay\n",
        " F2 / F5      Pause / resume NMEA log\n",
        " F3           Save config to flash ($PCAS00)\n",
        " F4           Cold restart (with confirmation)\n",
        " F6           Clear NMEA log\n\n",
        " Press F1 or Esc to close",
    );
    let block = Block::default().title(" Help ").borders(Borders::ALL)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(Paragraph::new(text).block(block), popup);
}

fn render_confirm(frame: &mut Frame, area: ratatui::layout::Rect, mode: dx_gp21_core::command::RestartMode) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    use ratatui::layout::Rect;

    let w = 52u16.min(area.width.saturating_sub(4));
    let h = 7u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x: area.x + x, y: area.y + y, width: w, height: h };

    let label = match mode {
        dx_gp21_core::command::RestartMode::Hot     => "Hot Restart",
        dx_gp21_core::command::RestartMode::Warm    => "Warm Restart",
        dx_gp21_core::command::RestartMode::Cold    => "Cold Restart",
        dx_gp21_core::command::RestartMode::Factory => "Factory Reset",
    };

    frame.render_widget(Clear, popup);
    let text = format!("\n Send {}?\n\n [Y] Confirm     [any other key] Cancel", label);
    let block = Block::default().title(format!(" Confirm: {label} ")).borders(Borders::ALL)
        .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
    frame.render_widget(Paragraph::new(text).block(block), popup);
}
