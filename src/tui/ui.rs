use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::app::{App, DetailTab};
use super::export::{ExportApp, ExportField, ExportFormat, ExportStatus};
use super::welcome::{FocusedField, LangOption, WelcomeApp};


// ─── Main View ───────────────────────────────────────────────────────────────

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let main_area = chunks[0];
    let help_area = chunks[1];

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_area);

    draw_tree(frame, app, main_chunks[0]);
    draw_detail(frame, app, main_chunks[1]);
    draw_help(frame, app, help_area);
}

fn draw_tree(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .enumerate()
        .map(|(i, &node_idx)| {
            let node = &app.nodes[node_idx];
            let indent = "  ".repeat(node.depth as usize);
            let arrow = if node.has_children {
                if node.expanded { "▼ " } else { "▶ " }
            } else {
                "  "
            };

            let icon = match node.depth {
                0 => "📦 ",
                1 => {
                    if node.label.starts_with("fn ") {
                        "ƒ  "
                    } else {
                        "◆  "
                    }
                }
                _ => "   ",
            };

            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                match node.depth {
                    0 => Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    1 => Style::default().fg(Color::Green),
                    _ => Style::default().fg(Color::White),
                }
            };

            let text = format!("{}{}{}{}", indent, arrow, icon, node.label);
            ListItem::new(text).style(style)
        })
        .collect();

    let stats = format!(
        " {} modules, {} types ",
        app.project.modules.len(),
        app.project
            .modules
            .iter()
            .map(|m| m.types.len())
            .sum::<usize>()
    );

    let tree = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Structure ")
            .title_bottom(stats)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(tree, area);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let tab_title = match app.tab {
        DetailTab::Machine => " Detail [Machine Context] ",
        DetailTab::Mermaid => " Detail [Mermaid] ",
    };

    let content = if let Some(node) = app.selected_node() {
        match app.tab {
            DetailTab::Machine => &node.detail,
            DetailTab::Mermaid => &node.detail, // For now both show same detail
        }
    } else {
        "No item selected"
    };

    // Syntax highlight the detail content
    let lines: Vec<Line> = content
        .lines()
        .map(|line| {
            if line.starts_with('@') || line.contains("@fn") || line.contains("@type") {
                highlight_machine_line(line)
            } else if line.starts_with("  →") || line.starts_with("  ->") {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Magenta),
                ))
            } else if line.starts_with("---") {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("Fields:")
                || line.starts_with("Methods")
                || line.starts_with("Call graph:")
            {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(line.to_string())
            }
        })
        .collect();

    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(tab_title)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(detail, area);
}

fn highlight_machine_line(line: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let remaining = line.to_string();

    // Highlight @ tags
    let tag_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default().fg(Color::White);
    let call_style = Style::default().fg(Color::Magenta);

    if let Some(calls_idx) = remaining.find("@calls[") {
        let before = &remaining[..calls_idx];
        let calls_part = &remaining[calls_idx..];

        // Highlight the part before @calls
        highlight_tags(before, &mut spans, tag_style, value_style);

        // Highlight @calls specially
        spans.push(Span::styled(calls_part.to_string(), call_style));
    } else {
        highlight_tags(&remaining, &mut spans, tag_style, value_style);
    }

    Line::from(spans)
}

fn highlight_tags(
    text: &str,
    spans: &mut Vec<Span<'static>>,
    tag_style: Style,
    value_style: Style,
) {
    let mut i = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    while i < len {
        if bytes[i] == b'@' {
            // Find end of tag word
            let start = i;
            i += 1;
            while i < len && bytes[i] != b' ' && bytes[i] != b'[' {
                i += 1;
            }
            spans.push(Span::styled(text[start..i].to_string(), tag_style));
        } else {
            let start = i;
            while i < len && bytes[i] != b'@' {
                i += 1;
            }
            spans.push(Span::styled(text[start..i].to_string(), value_style));
        }
    }
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let tab_indicator = match app.tab {
        DetailTab::Machine => "[Machine]  Mermaid ",
        DetailTab::Mermaid => " Machine  [Mermaid]",
    };

    let help = Line::from(vec![
        Span::styled(" ↑↓/jk ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Navigate  "),
        Span::styled("←→/hl ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Expand  "),
        Span::styled("Tab ", Style::default().fg(Color::Cyan).bold()),
        Span::raw(tab_indicator),
        Span::raw("  "),
        Span::styled("e ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Export  ", Style::default().fg(Color::White)),
        Span::styled("q ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Quit"),
    ]);

    let help_bar = Paragraph::new(help).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(help_bar, area);
}

// ─── Welcome Screen ───────────────────────────────────────────────────────────

/// Draw the welcome/input screen.
pub fn draw_welcome(frame: &mut Frame, app: &WelcomeApp) {
    let area = frame.area();

    // Fill background with dark color
    let bg = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 20)));
    frame.render_widget(bg, area);

    // Center a form box
    // Height = 28 → inner after borders = 26, after margin(1) = 24 available
    // Constraint sum = 21, Min(0) gets 3 → no overflow
    let form_area = centered_rect(70, 28, area);

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" skelecode ")
        .title_alignment(Alignment::Center)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let inner = outer_block.inner(form_area);
    frame.render_widget(outer_block, form_area);

    // Vertical layout inside the form
    // [0] banner  [1] subtitle  [2] spacer  [3] path
    // [4] spacer  [5] lang      [6] spacer  [7] exclude
    // [8] error   [9] confirm   [10] Min    [11] help
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // [0] banner (3 text lines)
            Constraint::Length(1), // [1] subtitle
            Constraint::Length(1), // [2] spacer
            Constraint::Length(3), // [3] path input
            Constraint::Length(1), // [4] spacer
            Constraint::Length(3), // [5] lang selector
            Constraint::Length(1), // [6] spacer
            Constraint::Length(3), // [7] exclude input
            Constraint::Length(1), // [8] error line
            Constraint::Length(3), // [9] confirm button
            Constraint::Min(0),    // [10] padding
            Constraint::Length(1), // [11] help
        ])
        .split(inner);

    draw_banner(frame, chunks[0]);
    draw_subtitle(frame, chunks[1]);
    draw_path_field(frame, app, chunks[3]);
    draw_lang_selector(frame, app, chunks[5]);
    draw_exclude_field(frame, app, chunks[7]);
    draw_error(frame, app, chunks[8]);
    draw_confirm_button(frame, app, chunks[9]);
    draw_welcome_help(frame, chunks[11]);
}

/// ASCII banner for skelecode — 3 text lines (matches Constraint::Length(3)).
fn draw_banner(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "  ╔═╗╦╔═╔═╗╦  ╔═╗╔═╗╔═╗╔╦╗╔═╗",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  ╚═╗╠╩╗║╣ ║  ║╣ ║  ║ ║ ║║║╣ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  ╚═╝╩ ╩╚═╝╩═╝╚═╝╚═╝╚═╝═╩╝╚═╝",
            Style::default().fg(Color::Rgb(0, 200, 200)).add_modifier(Modifier::BOLD),
        )),
    ];

    let banner = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(banner, area);
}

fn draw_subtitle(frame: &mut Frame, area: Rect) {
    let subtitle = Paragraph::new(
        "Code structure scanner · Generate context graphs for humans and AI",
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(subtitle, area);
}

/// Render a text input field with label + input box.
fn draw_text_input(
    frame: &mut Frame,
    label: &str,
    value: &str,
    focused: bool,
    area: Rect,
) {
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::Rgb(80, 80, 100)
    };
    let label_color = if focused { Color::Cyan } else { Color::Gray };

    // Cursor indicator at the end when focused
    let display_value = if focused {
        format!("{}▌", value)
    } else {
        value.to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", label))
        .title_style(
            Style::default()
                .fg(label_color)
                .add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }),
        )
        .border_style(Style::default().fg(border_color));

    let input = Paragraph::new(display_value)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(input, area);
}

fn draw_path_field(frame: &mut Frame, app: &WelcomeApp, area: Rect) {
    let focused = app.focused == FocusedField::PathInput;
    draw_text_input(frame, "📁 Project Path", &app.path_input, focused, area);
}

fn draw_exclude_field(frame: &mut Frame, app: &WelcomeApp, area: Rect) {
    let focused = app.focused == FocusedField::ExcludeInput;
    draw_text_input(
        frame,
        "⊘  Exclude Patterns  (comma-separated, optional)",
        &app.exclude_input,
        focused,
        area,
    );
}

fn draw_lang_selector(frame: &mut Frame, app: &WelcomeApp, area: Rect) {
    let focused = app.focused == FocusedField::LangSelector;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::Rgb(80, 80, 100)
    };
    let label_color = if focused { Color::Cyan } else { Color::Gray };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 🌐 Language ")
        .title_style(
            Style::default()
                .fg(label_color)
                .add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }),
        )
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build language option spans
    let mut spans: Vec<Span> = Vec::new();
    if focused {
        spans.push(Span::styled(" ◀ ", Style::default().fg(Color::DarkGray)));
    } else {
        spans.push(Span::raw("   "));
    }

    for (i, &opt) in LangOption::ALL_OPTIONS.iter().enumerate() {
        let is_selected = i == app.lang_index;
        let style = if is_selected && focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let label = if is_selected {
            format!(" [{}] ", opt.label())
        } else {
            format!("  {}  ", opt.label())
        };
        spans.push(Span::styled(label, style));
    }

    if focused {
        spans.push(Span::styled(" ▶", Style::default().fg(Color::DarkGray)));
    }

    let lang_line = Paragraph::new(Line::from(spans));

    // Center vertically (1 line in a 1-row inner)
    frame.render_widget(lang_line, inner);
}

fn draw_error(frame: &mut Frame, app: &WelcomeApp, area: Rect) {
    if let Some(ref msg) = app.error_msg {
        let err = Paragraph::new(msg.as_str())
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(err, area);
    }
}

fn draw_confirm_button(frame: &mut Frame, app: &WelcomeApp, area: Rect) {
    let focused = app.focused == FocusedField::ConfirmButton;

    let (btn_style, label) = if focused {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            "  ✔  Confirm & Scan  ",
        )
    } else {
        (
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            "  ✔  Confirm & Scan  ",
        )
    };

    let border_color = if focused {
        Color::Cyan
    } else {
        Color::Rgb(80, 80, 100)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let btn = Paragraph::new(Line::from(vec![Span::styled(label, btn_style)]))
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(btn, area);
}

fn draw_welcome_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" Tab ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓ ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Move  ", Style::default().fg(Color::DarkGray)),
        Span::styled("←→ ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Select Language  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Confirm  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Quit", Style::default().fg(Color::DarkGray)),
    ]);

    let help_bar = Paragraph::new(help).alignment(Alignment::Center);
    frame.render_widget(help_bar, area);
}

/// Return a centered Rect of fixed height inside `r`.
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let available_height = r.height;
    let top_pad = available_height.saturating_sub(height) / 2;

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_pad),
            Constraint::Length(height.min(available_height)),
            Constraint::Min(0),
        ])
        .split(r);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

// ─── Export Overlay ──────────────────────────────────────────────────────

/// Draw the export overlay on top of the main view.
pub fn draw_export_overlay(frame: &mut Frame, export: &ExportApp) {
    use ratatui::widgets::Clear;

    // Overlay box: 58% wide, 18 rows tall
    // inner after borders = 16, after margin(1) = 14 available
    // constraints sum = 3+1+3+1+3+1+1 = 13, Min(0) gets 1
    let area = centered_rect(58, 18, frame.area());

    // Clear the area behind the popup
    frame.render_widget(Clear, area);

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" ↗ Export ")
        .title_alignment(Alignment::Center)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Rgb(12, 12, 25)));

    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Layout: 18 - 2 borders = 16, - 2 margins = 14
    // [0] format(3) [1] space(1) [2] path(3) [3] status(1) [4] button(3) [5] Min [6] help(1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // [0] format selector
            Constraint::Length(1), // [1] spacer
            Constraint::Length(3), // [2] path input
            Constraint::Length(1), // [3] status line
            Constraint::Length(3), // [4] export button
            Constraint::Min(0),    // [5] padding
            Constraint::Length(1), // [6] help
        ])
        .split(inner);

    draw_export_format(frame, export, chunks[0]);
    draw_export_path(frame, export, chunks[2]);
    draw_export_status(frame, export, chunks[3]);
    draw_export_button(frame, export, chunks[4]);
    draw_export_help(frame, chunks[6]);
}

fn draw_export_format(frame: &mut Frame, export: &ExportApp, area: Rect) {
    let focused = export.focused == ExportField::FormatSelector;
    let border_color = if focused { Color::Yellow } else { Color::Rgb(80, 80, 100) };
    let label_color = if focused { Color::Yellow } else { Color::Gray };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Format ")
        .title_style(
            Style::default()
                .fg(label_color)
                .add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }),
        )
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans: Vec<Span> = Vec::new();
    if focused {
        spans.push(Span::styled(" ◄ ", Style::default().fg(Color::DarkGray)));
    } else {
        spans.push(Span::raw("   "));
    }

    for (i, &fmt) in ExportFormat::ALL.iter().enumerate() {
        let is_sel = i == export.format_index;
        let style = if is_sel && focused {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if is_sel {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(
            if is_sel { format!(" [{}] ", fmt.label()) } else { format!("  {}  ", fmt.label()) },
            style,
        ));
    }

    if focused {
        spans.push(Span::styled(" ►", Style::default().fg(Color::DarkGray)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn draw_export_path(frame: &mut Frame, export: &ExportApp, area: Rect) {
    let focused = export.focused == ExportField::PathInput;
    let border_color = if focused { Color::Yellow } else { Color::Rgb(80, 80, 100) };
    let label_color = if focused { Color::Yellow } else { Color::Gray };

    let display = if focused {
        format!("{}▌", export.path_input)
    } else {
        export.path_input.clone()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Output File ")
        .title_style(
            Style::default()
                .fg(label_color)
                .add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }),
        )
        .border_style(Style::default().fg(border_color));

    frame.render_widget(
        Paragraph::new(display).block(block).style(Style::default().fg(Color::White)),
        area,
    );
}

fn draw_export_status(frame: &mut Frame, export: &ExportApp, area: Rect) {
    if let Some(ref status) = export.status {
        let (msg, style) = match status {
            ExportStatus::Success(s) => (
                format!("✔  {}", s),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            ExportStatus::Error(e) => (
                format!("⚠  {}", e),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        frame.render_widget(
            Paragraph::new(msg).style(style).alignment(Alignment::Center),
            area,
        );
    }
}

fn draw_export_button(frame: &mut Frame, export: &ExportApp, area: Rect) {
    let focused = export.focused == ExportField::ExportButton;
    let (btn_style, label) = if focused {
        (
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
            "  ↗  Export to File  ",
        )
    } else {
        (
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            "  ↗  Export to File  ",
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused { Color::Yellow } else { Color::Rgb(80, 80, 100) }));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label, btn_style)))
            .block(block)
            .alignment(Alignment::Center),
        area,
    );
}

fn draw_export_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("Tab ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("↔ ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Format  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Export  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help).alignment(Alignment::Center), area);
}
