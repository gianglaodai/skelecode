use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::app::{App, DetailTab};
use super::export::{ExportApp, ExportField, ExportFormat, ExportStatus};
use super::welcome::{FocusedField, LangOption, WelcomeApp};


// ─── Main View ───────────────────────────────────────────────────────────────

pub fn draw(frame: &mut Frame, app: &mut App) {
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

    if app.search_mode || !app.search_query.is_empty() {
        draw_search_bar(frame, app, help_area);
    } else {
        draw_help(frame, app, help_area);
    }
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let selected = app.selected;
    let query = app.search_query.to_lowercase();
    let searching = !query.is_empty();

    let items: Vec<ListItem> = app
        .visible
        .iter()
        .enumerate()
        .map(|(i, &node_idx)| {
            let node = &app.nodes[node_idx];
            let indent = "  ".repeat(node.depth as usize);
            let arrow = if node.has_children && !searching {
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

            let prefix = format!("{}{}{}", indent, arrow, icon);
            let is_selected = i == selected;
            let is_match = searching && node.label.to_lowercase().contains(&query);

            if is_selected {
                // Selected row: render as a single styled string
                let text = format!("{}{}", prefix, node.label);
                ListItem::new(text).style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else if is_match && searching {
                // Highlight the matched portion of the label
                let label = &node.label;
                let label_lower = label.to_lowercase();
                let base_style = match node.depth {
                    0 => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    1 => Style::default().fg(Color::Green),
                    _ => Style::default().fg(Color::White),
                };
                let mut spans: Vec<Span> = vec![Span::styled(prefix, base_style)];

                // Split label around the first match and highlight it
                if let Some(pos) = label_lower.find(&query) {
                    let before = &label[..pos];
                    let matched = &label[pos..pos + query.len()];
                    let after = &label[pos + query.len()..];
                    if !before.is_empty() {
                        spans.push(Span::styled(before.to_string(), base_style));
                    }
                    spans.push(Span::styled(
                        matched.to_string(),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                    if !after.is_empty() {
                        spans.push(Span::styled(after.to_string(), base_style));
                    }
                } else {
                    spans.push(Span::styled(label.to_string(), base_style));
                }

                ListItem::new(Line::from(spans))
            } else {
                // Normal unselected, non-matching row
                let style = match node.depth {
                    0 => Style::default()
                        .fg(if searching { Color::DarkGray } else { Color::Yellow })
                        .add_modifier(if searching { Modifier::empty() } else { Modifier::BOLD }),
                    1 => Style::default().fg(if searching { Color::DarkGray } else { Color::Green }),
                    _ => Style::default().fg(Color::DarkGray),
                };
                let text = format!("{}{}", prefix, node.label);
                ListItem::new(text).style(style)
            }
        })
        .collect();

    let match_count = if searching {
        app.visible.iter()
            .filter(|&&i| app.nodes[i].label.to_lowercase().contains(&query))
            .count()
    } else {
        0
    };

    let stats = if searching {
        format!(" {} match{} ", match_count, if match_count == 1 { "" } else { "es" })
    } else {
        format!(
            " {} modules, {} types ",
            app.project.modules.len(),
            app.project.modules.iter().map(|m| m.types.len()).sum::<usize>()
        )
    };

    let title = if searching {
        format!(" Structure  [/{}] ", app.search_query)
    } else {
        " Structure ".to_string()
    };

    let border_color = if searching { Color::Yellow } else { Color::Cyan };

    let tree = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_bottom(stats)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(Style::default());

    frame.render_stateful_widget(tree, area, &mut app.list_state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let tab_title = match app.tab {
        DetailTab::Machine => " Detail [Machine Context] ",
        DetailTab::Obsidian => " Detail [Obsidian Preview] ",
    };

    let content: String = if let Some(node) = app.selected_node() {
        match app.tab {
            DetailTab::Machine => node.detail_machine.clone(),
            DetailTab::Obsidian => {
                if node.detail_obsidian.is_empty() {
                    format!("(No Obsidian preview for {})", node.label)
                } else {
                    node.detail_obsidian.clone()
                }
            }
        }
    } else {
        String::new()
    };

    // Syntax highlight the detail content
    let lines: Vec<Line> = content
        .as_str()
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
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

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
        DetailTab::Machine => "[Machine]  Obsidian ",
        DetailTab::Obsidian => " Machine  [Obsidian]",
    };

    let help = Line::from(vec![
        Span::styled(" ↑↓/jk ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Nav  "),
        Span::styled("←→/hl ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Expand  "),
        Span::styled("/ ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Search  "),
        Span::styled("u/d ", Style::default().fg(Color::Green).bold()),
        Span::raw("Scroll  "),
        Span::styled("Tab ", Style::default().fg(Color::Cyan).bold()),
        Span::raw(tab_indicator),
        Span::raw("  "),
        Span::styled("y ", Style::default().fg(Color::Green).bold()),
        Span::raw("Copy  "),
        Span::styled("e ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Export  ", Style::default().fg(Color::White)),
        Span::styled("b ", Style::default().fg(Color::Cyan).bold()),
        Span::raw("Back  "),
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

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let query_display = if app.search_mode {
        format!("{}▌", app.search_query)
    } else {
        app.search_query.clone()
    };

    let match_count = app.visible.iter()
        .filter(|&&i| app.nodes[i].label.to_lowercase().contains(&app.search_query.to_lowercase()))
        .count();

    let status = if app.search_query.is_empty() {
        String::new()
    } else {
        format!("  {} match{}", match_count, if match_count == 1 { "" } else { "es" })
    };

    let hint = if app.search_mode {
        "  Enter Confirm  Esc Clear"
    } else {
        "  / Search again  Esc Clear filter"
    };

    let bar = Line::from(vec![
        Span::styled(" Search: ", Style::default().fg(Color::Yellow).bold()),
        Span::styled(
            query_display,
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(status, Style::default().fg(Color::Green).bold()),
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ]);

    let bar_widget = Paragraph::new(bar).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(bar_widget, area);
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
        .title(" Output Path/Dir ")
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
            "  ↗  Export  ",
        )
    } else {
        (
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            "  ↗  Export  ",
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
