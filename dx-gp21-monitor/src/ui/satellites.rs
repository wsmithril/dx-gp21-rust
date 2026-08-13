use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use dx_gp21_core::types::GnssSystem;

use dx_gp21::{GnssSession, GnssStore};

use crate::app::App;

fn snr_bar(snr: u8, width: usize) -> (String, Style) {
    let filled = ((snr as usize * width) / 50).min(width);
    let bar = "█".repeat(filled) + &"░".repeat(width - filled);
    let style = if snr >= 30 {
        Style::default().fg(Color::Green)
    } else if snr >= 20 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    };
    (bar, style)
}

fn sys_color(sys: GnssSystem) -> Color {
    match sys {
        GnssSystem::Gps => Color::Green,
        GnssSystem::Beidou => Color::Red,
        GnssSystem::Glonass => Color::Blue,
        GnssSystem::Galileo => Color::Magenta,
        GnssSystem::Qzss => Color::Cyan,
        GnssSystem::Multi => Color::White,
    }
}

pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let state = app.session.state();
    let all_sats = state.satellites();
    let used = state.sats_used_count();
    let in_view = state.sats_in_view_count();

    // Sort: used first, then by SNR descending
    let mut sats: Vec<_> = all_sats.iter().collect();
    sats.sort_by(|a, b| {
        b.used.cmp(&a.used)
            .then(b.snr.unwrap_or(0).cmp(&a.snr.unwrap_or(0)))
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Satellites   {} used / {} in view ", used, in_view));
    let inner = block.inner(area);
    let visible_rows = inner.height as usize;

    let bar_width = (inner.width as usize).saturating_sub(36).clamp(10, 28);

    let header = Line::from(vec![
        Span::styled(
            format!("  {:>4}  {:3}  {:3}  {:3}  {:>2}  {:width$}  Used",
                "ID", "Sys", "El°", "Az°", "SNR", "", width = bar_width),
            Style::default().fg(Color::Gray),
        ),
    ]);

    let mut lines = vec![header];

    let sats_to_show = sats.iter()
        .skip(app.sat_scroll)
        .take(visible_rows.saturating_sub(2));

    for sat in sats_to_show {
        let sys_str = sat.system.label();
        let el_str = sat.elevation.map(|e| format!("{:3}", e)).unwrap_or("  -".into());
        let az_str = sat.azimuth.map(|a| format!("{:3}", a)).unwrap_or("  -".into());
        let snr_val = sat.snr.unwrap_or(0);
        let snr_str = if sat.snr.is_some() { format!("{:2}", snr_val) } else { " -".into() };
        let (bar, bar_style) = if sat.snr.is_some() {
            snr_bar(snr_val, bar_width)
        } else {
            ("░".repeat(bar_width), Style::default().fg(Color::DarkGray))
        };
        let used_marker = if sat.used { "●" } else { "·" };
        let used_style = if sat.used {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let line = Line::from(vec![
            Span::raw(format!("  {:>4}  ", sat.svid)),
            Span::styled(format!("{:3}  ", sys_str), Style::default().fg(sys_color(sat.system))),
            Span::raw(format!("{:3}  {:3}  {:>2}  ", el_str, az_str, snr_str)),
            Span::styled(bar, bar_style),
            Span::raw("  "),
            Span::styled(used_marker, used_style),
        ]);
        lines.push(line);
    }

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}
