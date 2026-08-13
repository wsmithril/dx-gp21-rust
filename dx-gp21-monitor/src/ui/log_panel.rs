use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use dx_gp21::GnssSession;

use crate::app::{App, LogEntry};

fn sentence_color(raw: &str) -> Color {
    if raw.contains("GGA") { Color::Green }
    else if raw.contains("RMC") { Color::Cyan }
    else if raw.contains("GSA") { Color::Yellow }
    else if raw.contains("GSV") { Color::Blue }
    else if raw.contains("TXT") { Color::Magenta }
    else { Color::White }
}

pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let paused = app.log_paused;

    let pause_str = if paused { "  [PAUSED]" } else { "" };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" NMEA Log{pause_str} "))
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    let visible = inner.height as usize;
    let total = app.log.len();
    let start = total.saturating_sub(visible);

    let lines: Vec<Line> = app.log.iter()
        .skip(start)
        .take(visible)
        .map(|entry: &LogEntry| {
            let color = if paused {
                Color::DarkGray
            } else if !entry.valid {
                // Bad checksum or unknown sentence — show in red
                Color::Red
            } else {
                sentence_color(&entry.raw)
            };
            Line::from(Span::styled(entry.raw.as_str(), Style::default().fg(color)))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
