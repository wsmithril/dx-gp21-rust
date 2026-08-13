use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use dx_gp21::{GnssSession, GnssStore};
use dx_gp21_core::types::AntennaStatus;

use crate::app::App;

fn label_style() -> Style { Style::default().fg(Color::Gray) }
fn value_style() -> Style { Style::default().fg(Color::White) }

fn fmt_deg(deg: f64, pos_char: char, neg_char: char) -> String {
    let hemi = if deg >= 0.0 { pos_char } else { neg_char };
    let abs = deg.abs();
    let d = abs as u32;
    let min = (abs - d as f64) * 60.0;
    format!("{:3}° {:07.4}′ {}", d, min, hemi)
}


/// Left column: Position + Velocity + Antenna
pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let state = app.session.state();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Length(5), Constraint::Min(1)])
        .split(area);

    // ── POSITION ──────────────────────────────────────────────────────────────
    let mut pos_lines = vec![Line::from(Span::styled("POSITION", Style::default().add_modifier(Modifier::BOLD).fg(Color::White)))];

    if let Some(gga) = state.gga() {
        let coord_style = Style::default().fg(Color::Cyan);
        let alt_style   = Style::default().fg(Color::Green);
        pos_lines.push(Line::from(vec![
            Span::styled(" Lat  ", label_style()),
            Span::styled(fmt_deg(gga.lat, 'N', 'S'), coord_style),
        ]));
        pos_lines.push(Line::from(vec![
            Span::styled(" Lon  ", label_style()),
            Span::styled(fmt_deg(gga.lon, 'E', 'W'), coord_style),
        ]));
        pos_lines.push(Line::from(vec![
            Span::styled(" Alt   ", label_style()),
            Span::styled(format!("{:8.1} m", gga.alt_msl), alt_style),
            Span::styled(" MSL", label_style()),
        ]));
        pos_lines.push(Line::from(vec![
            Span::styled(" Geoid ", label_style()),
            Span::styled(format!("{:+8.1} m", gga.geoid_sep), value_style()),
        ]));
        let fix_str = match gga.fix_quality {
            dx_gp21_core::types::FixQuality::Sps       => ("SPS fix", Color::Green),
            dx_gp21_core::types::FixQuality::Estimated => ("DR / estimated", Color::Yellow),
            dx_gp21_core::types::FixQuality::Invalid   => ("Invalid", Color::Red),
        };
        pos_lines.push(Line::from(vec![
            Span::styled(" Fix  ", label_style()),
            Span::styled(fix_str.0, Style::default().fg(fix_str.1)),
        ]));
    } else {
        pos_lines.push(Line::from(Span::styled(
            " Waiting for fix…",
            Style::default().fg(Color::Gray),
        )));
    }

    let pos_block = Block::default().borders(Borders::ALL).title(" Position ");
    frame.render_widget(Paragraph::new(pos_lines).block(pos_block), sections[0]);

    // ── VELOCITY ──────────────────────────────────────────────────────────────
    let mut vel_lines = vec![];
    if let Some(vtg) = state.vtg() {
        let spd_color = if vtg.speed_kmh > 5.0 { Color::Yellow } else { Color::White };
        vel_lines.push(Line::from(vec![
            Span::styled(" Ground  ", label_style()),
            Span::styled(format!("{:6.2}", vtg.speed_knots), Style::default().fg(spd_color)),
            Span::styled(" kn", label_style()),
        ]));
        vel_lines.push(Line::from(vec![
            Span::styled(" Ground  ", label_style()),
            Span::styled(format!("{:6.2}", vtg.speed_kmh), Style::default().fg(spd_color)),
            Span::styled(" km/h", label_style()),
        ]));
        vel_lines.push(Line::from(vec![
            Span::styled(" Course  ", label_style()),
            Span::styled(format!("{:6.2}°", vtg.course_true), Style::default().fg(Color::Cyan)),
            Span::styled("  T", label_style()),
        ]));
    }
    if let Some(dhv) = state.dhv() {
        vel_lines.push(Line::from(vec![
            Span::styled(" 3D Spd  ", label_style()),
            Span::styled(format!("{:6.2}", dhv.speed_3d), Style::default().fg(Color::Magenta)),
            Span::styled(" m/s", label_style()),
        ]));
    }
    if vel_lines.is_empty() {
        vel_lines.push(Line::from(Span::styled(" —", Style::default().fg(Color::Gray))));
    }
    let vel_block = Block::default().borders(Borders::ALL).title(" Velocity ");
    frame.render_widget(Paragraph::new(vel_lines).block(vel_block), sections[1]);

    // ── ANTENNA ───────────────────────────────────────────────────────────────
    let (ant_text, ant_style) = match state.antenna() {
        AntennaStatus::Ok => ("OK", Style::default().fg(Color::Green)),
        AntennaStatus::Open => ("OPEN", Style::default().fg(Color::Yellow)),
        AntennaStatus::Short => ("SHORT", Style::default().fg(Color::Red)),
        AntennaStatus::Unknown => ("?", Style::default().fg(Color::Gray)),
    };
    let ant_line = Line::from(vec![
        Span::styled(" Status  ", label_style()),
        Span::styled(ant_text, ant_style),
    ]);
    let ant_block = Block::default().borders(Borders::ALL).title(" Antenna ");
    frame.render_widget(Paragraph::new(vec![ant_line]).block(ant_block), sections[2]);
}

/// Right column: Accuracy + Configuration
pub fn render_right<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let state = app.session.state();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // ── ACCURACY ──────────────────────────────────────────────────────────────
    let dop = state.dop();
    let mut acc_lines = vec![];

    fn dop_line(label: &str, v: f32) -> Line<'static> {
        let filled = ((v.min(10.0) / 10.0 * 14.0) as usize).min(14);
        let bar: String = "█".repeat(filled) + &"░".repeat(14 - filled);
        let (quality, style) = match v {
            d if d < 2.0 => ("Excellent", Style::default().fg(Color::Green)),
            d if d < 5.0 => ("Good", Style::default().fg(Color::Cyan)),
            d if d < 10.0 => ("Moderate", Style::default().fg(Color::Yellow)),
            _ => ("Poor", Style::default().fg(Color::Red)),
        };
        Line::from(vec![
            Span::styled(format!(" {:5} {:4.1}  ", label, v), Style::default().fg(Color::Gray)),
            Span::styled(bar, style),
            Span::raw(" "),
            Span::styled(quality, style),
        ])
    }

    if dop.hdop > 0.0 {
        acc_lines.push(dop_line("HDOP", dop.hdop));
        acc_lines.push(dop_line("VDOP", dop.vdop));
        acc_lines.push(dop_line("PDOP", dop.pdop));
    }

    if let Some(gst) = state.gst() {
        acc_lines.push(Line::from(vec![
            Span::styled(" Lat σ  ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1} m", gst.std_lat), value_style()),
        ]));
        acc_lines.push(Line::from(vec![
            Span::styled(" Lon σ  ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1} m", gst.std_lon), value_style()),
        ]));
        acc_lines.push(Line::from(vec![
            Span::styled(" Alt σ  ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1} m", gst.std_alt), value_style()),
        ]));
        acc_lines.push(Line::from(vec![
            Span::styled(" RMS    ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1} m", gst.rms), value_style()),
        ]));
    }

    if acc_lines.is_empty() {
        acc_lines.push(Line::from(Span::styled(" Waiting for DOP…", Style::default().fg(Color::Gray))));
    }

    let acc_block = Block::default().borders(Borders::ALL).title(" Accuracy ");
    frame.render_widget(Paragraph::new(acc_lines).block(acc_block), sections[0]);

    // ── CONFIGURATION ─────────────────────────────────────────────────────────
    let baud = state.baud_rate;
    let rate = state.update_rate;
    let mask = state.system_mask;

    let cfg_lines = vec![
        Line::from(vec![
            Span::styled(" Baud        ", label_style()),
            Span::styled(format!("{baud}"), value_style()),
        ]),
        Line::from(vec![
            Span::styled(" Update Rate ", label_style()),
            Span::styled(format!("{rate}"), value_style()),
        ]),
        Line::from(vec![
            Span::styled(" Systems     ", label_style()),
            Span::styled(format!("{mask}"), value_style()),
        ]),
        Line::from(vec![
            Span::styled(" Protocol    ", label_style()),
            Span::styled("NMEA 4.1+", value_style()),
        ]),
    ];

    let cfg_block = Block::default().borders(Borders::ALL).title(" Configuration ");
    frame.render_widget(Paragraph::new(cfg_lines).block(cfg_block), sections[1]);
}
