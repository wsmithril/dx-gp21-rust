use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use dx_gp21_core::types::GnssSystem;

use dx_gp21::{GnssSession, GnssStore};

use crate::app::App;

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

fn sys_char(sys: GnssSystem) -> char {
    match sys {
        GnssSystem::Gps => 'G',
        GnssSystem::Beidou => 'B',
        GnssSystem::Glonass => 'R',
        GnssSystem::Galileo => 'E',
        GnssSystem::Qzss => 'Q',
        GnssSystem::Multi => '?',
    }
}

pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let state = app.session.state();
    let sats = state.satellites();

    let block = Block::default().borders(Borders::ALL).title(" Sky Plot ");
    let inner = block.inner(area);

    let w = inner.width as usize;
    let h = inner.height as usize;
    if w < 5 || h < 5 {
        frame.render_widget(block, area);
        return;
    }

    // Draw into a character grid
    // Each cell: (char, color)
    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', Color::Reset); w]; h];

    let cx = (w as f64) / 2.0;
    let cy = (h as f64) / 2.0;
    // radius in chars: leave 1-char margin; chars are ~2:1 height:width, so scale y
    let rx = (w as f64 / 2.0) - 1.5;
    let ry = (h as f64 / 2.0) - 1.0;

    // Draw concentric elevation rings at 0°, 30°, 60°
    for &elev in &[0u32, 30, 60] {
        let frac = (90 - elev) as f64 / 90.0;
        let ring_rx = rx * frac;
        let ring_ry = ry * frac;
        // Draw ellipse outline
        for step in 0..360usize {
            let angle = (step as f64).to_radians();
            let px = (cx + ring_rx * angle.sin()).round() as isize;
            let py = (cy - ring_ry * angle.cos()).round() as isize;
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h
                && grid[py as usize][px as usize].0 == ' ' {
                    grid[py as usize][px as usize] = ('·', Color::DarkGray);
                }
        }
    }

    // Cardinal labels
    let label_pairs: &[(f64, f64, char)] = &[
        (cx, 0.0, 'N'),
        (cx, h as f64 - 1.0, 'S'),
        (w as f64 - 1.5, cy, 'E'),
        (0.5, cy, 'W'),
    ];
    for &(lx, ly, c) in label_pairs {
        let px = lx.round() as usize;
        let py = ly.round() as usize;
        if px < w && py < h {
            grid[py][px] = (c, Color::DarkGray);
        }
    }

    // Plot satellites
    for sat in sats {
        let el = match sat.elevation { Some(e) => e as f64, None => continue };
        let az = match sat.azimuth  { Some(a) => a as f64, None => continue };
        // r = 0 at zenith (90°), r = 1 at horizon (0°)
        let frac = (90.0 - el) / 90.0;
        let az_rad = az.to_radians();
        let px = (cx + rx * frac * az_rad.sin()).round() as isize;
        let py = (cy - ry * frac * az_rad.cos()).round() as isize;
        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
            let color = if sat.used { sys_color(sat.system) } else { Color::DarkGray };
            let ch = if sat.used { sys_char(sat.system).to_ascii_uppercase() }
                else { sys_char(sat.system).to_ascii_lowercase() };
            grid[py as usize][px as usize] = (ch, color);
        }
    }

    // Convert grid to ratatui Lines
    let mut lines: Vec<Line> = grid.into_iter().map(|row| {
        let spans: Vec<Span> = row.into_iter().map(|(ch, color)| {
            Span::styled(ch.to_string(), Style::default().fg(color))
        }).collect();
        Line::from(spans)
    }).collect();

    // Legend at bottom of inner area
    let legend = Line::from(vec![
        Span::styled("G", Style::default().fg(Color::Green)),
        Span::raw("=GPS "),
        Span::styled("B", Style::default().fg(Color::Red)),
        Span::raw("=BDS "),
        Span::styled("R", Style::default().fg(Color::Blue)),
        Span::raw("=GLO "),
        Span::styled("E", Style::default().fg(Color::Magenta)),
        Span::raw("=GAL "),
        Span::styled("Q", Style::default().fg(Color::Cyan)),
        Span::raw("=QZS  upper=used lower=visible"),
    ]);
    if let Some(last) = lines.last_mut() {
        *last = legend;
    }

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}
