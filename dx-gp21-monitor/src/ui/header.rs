use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use dx_gp21::{GnssSession, GnssStore};
use dx_gp21_core::types::{FixMode, AntennaStatus};

use crate::app::App;

pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let state = app.session.state();

    let (fix_label, fix_bg, fix_fg) = match state.fix_mode() {
        FixMode::Fix3D => (" ● 3D FIX ", Color::Green,  Color::Black),
        FixMode::Fix2D => (" ● 2D FIX ", Color::Yellow, Color::Black),
        FixMode::NoFix => (" ○ NO FIX ", Color::Red,    Color::White),
    };

    let (ant_str, ant_color) = match state.antenna() {
        AntennaStatus::Ok    => (" ANT:OK",    Color::Green),
        AntennaStatus::Open  => (" ANT:OPEN",  Color::Yellow),
        AntennaStatus::Short => (" ANT:SHORT", Color::Red),
        AntennaStatus::Unknown => ("", Color::DarkGray),
    };

    let (utc_str, date_str) = if let Some(rmc) = state.rmc() {
        let t = rmc.time;
        let d = rmc.date;
        (
            format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second),
            format!("{:04}-{:02}-{:02}", d.year, d.month, d.day),
        )
    } else if let Some(gga) = state.gga() {
        let t = gga.time;
        (format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second), String::new())
    } else {
        ("--:--:--".into(), String::new())
    };

    let sats_used = state.sats_used_count();
    let sats_view = state.sats_in_view_count();
    let paused_span = if app.log_paused {
        Span::styled("  ⏸ PAUSED", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        Span::raw("")
    };

    // Show replay speed indicator for seekable (file replay) sessions
    let speed_span = if app.session.seekable() {
        Span::styled(
            format!("  ▶ {}", app.replay_speed_label()),
            Style::default().fg(Color::Cyan).bg(Color::DarkGray),
        )
    } else {
        Span::raw("")
    };

    let line = Line::from(vec![
        Span::styled(
            " DX-GP21 GNSS ",
            Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}  @{}bps ", app.port_name, state.baud_rate),
            Style::default().fg(Color::Gray).bg(Color::DarkGray),
        ),
        Span::raw(" "),
        Span::styled(fix_label, Style::default().fg(fix_fg).bg(fix_bg).add_modifier(Modifier::BOLD)),
        Span::styled(ant_str, Style::default().fg(ant_color).bg(Color::DarkGray)),
        Span::styled(
            format!("  {}/{} sats ", sats_used, sats_view),
            Style::default().fg(Color::Cyan).bg(Color::DarkGray),
        ),
        Span::raw(" │ "),
        Span::styled(utc_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" UTC ", Style::default().fg(Color::Gray)),
        Span::styled(date_str, Style::default().fg(Color::Gray)),
        paused_span,
        speed_span,
    ]);

    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::DarkGray)),
        area,
    );
}
