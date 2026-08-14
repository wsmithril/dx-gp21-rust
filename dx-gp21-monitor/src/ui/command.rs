use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use dx_gp21::GnssSession;

use crate::app::{App, CompletionRow, COMMANDS};

const CMD_COL: usize = 34; // fixed width for the command column in the popup

pub fn render<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    if app.session.is_readonly() {
        let block = Block::default().borders(Borders::ALL).title(" Command ")
            .border_style(Style::default().fg(Color::DarkGray));
        let msg = Line::from(Span::styled(
            " File playback mode — commands not available",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(Paragraph::new(msg).block(block), area);
        return;
    }

    // Split command area: [input + hint] | [response]
    let has_response = !app.response_lines.is_empty();
    let sections = if has_response {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(46)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(0)])
            .split(area)
    };

    render_input(frame, sections[0], app);
    if has_response {
        render_response(frame, sections[1], app);
    }

    // Completion popup — only when the user has started typing
    if app.should_show_completions() {
        let indices = app.matching_indices();
        if !indices.is_empty() {
            render_popup(frame, sections[0], app, &indices);
        }
    }
}

fn render_input<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Command  [Tab/Shift+Tab] complete  [↑↓] history  [Enter] send ")
        .border_style(Style::default().fg(Color::Cyan));

    let input_line = if app.cmd_input.is_empty() {
        Line::from(vec![
            Span::styled("❯ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("$", Style::default().fg(Color::DarkGray)),
            Span::styled("▌", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ])
    } else {
        Line::from(vec![
            Span::styled("❯ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(&app.cmd_input, Style::default().fg(Color::White)),
            Span::styled("▌", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ])
    };

    let hint = if let Some(ref msg) = app.status_msg {
        Line::from(Span::styled(msg.as_str(), Style::default().fg(Color::Yellow)))
    } else if app.cmd_input.is_empty() {
        Line::from(Span::styled(
            "  Type $PCAS… or a keyword (e.g. 'baud', 'restart', 'gps')",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        let n = app.matching_indices().len();
        let sel = app.completion_idx.map(|i| {
            COMMANDS[i].description
        }).unwrap_or("");
        let text = if n == 1 {
            format!("  {}", COMMANDS[app.matching_indices()[0]].category_label)
        } else if !sel.is_empty() {
            format!("  {sel}")
        } else {
            format!("  {n} matches — Tab to cycle")
        };
        Line::from(Span::styled(text, Style::default().fg(Color::Gray)))
    };

    frame.render_widget(Paragraph::new(vec![input_line, hint]).block(block), area);
}

fn render_response<S: GnssSession>(frame: &mut Frame, area: Rect, app: &App<S>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Response ")
        .border_style(Style::default().fg(Color::Yellow));

    let lines: Vec<Line> = app.response_lines.iter().map(|s| {
        Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Yellow)))
    }).collect();

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_popup<S: GnssSession>(frame: &mut Frame, input_area: Rect, app: &App<S>, indices: &[usize]) {
    let rows = app.completion_rows();

    // Single-match: show detail panel instead of the list
    if indices.len() == 1 {
        render_detail(frame, input_area, app, indices[0]);
        return;
    }

    // Multi-match list popup
    // Count visible rows (variants + headers), cap at 14 shown at once
    let total_rows = rows.len();
    let max_visible = 14usize;
    let popup_h = (total_rows.min(max_visible) + 2) as u16; // +2 for borders
    let popup_w = input_area.width.saturating_sub(2);

    if input_area.y < popup_h + 1 { return; }

    let popup = Rect {
        x: input_area.x + 1,
        y: input_area.y.saturating_sub(popup_h),
        width: popup_w,
        height: popup_h,
    };

    // Scroll to keep selected item visible
    let selected_row = app.completion_idx.and_then(|sel_idx| {
        rows.iter().enumerate().find_map(|(r, row)| {
            if let CompletionRow::Variant { idx } = row && *idx == sel_idx { return Some(r); }
            None
        })
    }).unwrap_or(0);

    let scroll = if selected_row >= max_visible { selected_row + 1 - max_visible } else { 0 };
    let visible_rows = rows.iter().skip(scroll).take(max_visible);

    let lines: Vec<Line> = visible_rows.map(|row| match row {
        CompletionRow::Header { category, label } => Line::from(vec![
            Span::styled(" ── ", Style::default().fg(Color::DarkGray)),
            Span::styled(*category, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!(": {label} ──"), Style::default().fg(Color::Gray)),
        ]),
        CompletionRow::Variant { idx } => {
            let cmd = &COMMANDS[*idx];
            let selected = app.completion_idx == Some(*idx);
            let (bg, cmd_fg, desc_fg) = if selected {
                (Color::Cyan, Color::Black, Color::Black)
            } else {
                (Color::Reset, Color::White, Color::Gray)
            };
            let style_bg = Style::default().bg(bg);
            let prefix = if selected { "▶ " } else { "  " };
            // Fixed-width command column, then description
            let cmd_text = format!("{:<width$}", cmd.full_command, width = CMD_COL);
            Line::from(vec![
                Span::styled(prefix, style_bg.fg(cmd_fg)),
                Span::styled(cmd_text, style_bg.fg(cmd_fg).add_modifier(Modifier::BOLD)),
                Span::styled(cmd.description, style_bg.fg(desc_fg)),
            ])
        }
    }).collect();

    frame.render_widget(Clear, popup);
    let block = Block::default().borders(Borders::ALL)
        .title(" Completions  [Tab] next  [Shift+Tab] prev  [Enter] send ")
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn render_detail<S: GnssSession>(frame: &mut Frame, input_area: Rect, _app: &App<S>, idx: usize) {
    let cmd = &COMMANDS[idx];

    // Detail panel: taller than the normal list
    let detail_lines: Vec<&str> = cmd.detail.lines().collect();
    let popup_h = (detail_lines.len() + 5).min(14) as u16;
    let popup_w = input_area.width.saturating_sub(2);

    if input_area.y < popup_h + 1 { return; }

    let popup = Rect {
        x: input_area.x + 1,
        y: input_area.y.saturating_sub(popup_h),
        width: popup_w,
        height: popup_h,
    };

    let mut lines = vec![
        // Command in large, prominent style
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(cmd.full_command, Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)),
            Span::styled(format!("   — {}", cmd.description),
                Style::default().fg(Color::White)),
        ]),
        Line::from(Span::raw("")),
    ];

    for line in detail_lines {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(Color::Gray),
        )));
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "  Press Enter to send, Esc to clear",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Clear, popup);
    let block = Block::default().borders(Borders::ALL)
        .title(format!(" {} — {} ", cmd.category, cmd.category_label))
        .border_style(Style::default().fg(Color::Green));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}
