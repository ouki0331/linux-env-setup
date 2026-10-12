use crate::app::App;
use crate::models::{Focus, Language};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn render(app: &App, frame: &mut Frame) {
    let size = frame.area();

    // Top: Main Area, Bottom: Footer Bar
    let root_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(1)])
        .split(size);

    let main_area = root_layout[0];
    let footer_area = root_layout[1];

    // Split main area horizontally: Left Panel (36%) vs Right Panel (64%)
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(main_area);

    let left_area = panels[0];
    let right_area = panels[1];

    render_left_panel(app, frame, left_area);
    render_right_panel(app, frame, right_area);
    render_footer(app, frame, footer_area);
}

fn render_left_panel(app: &App, frame: &mut Frame, area: Rect) {
    // Left panel vertical split: Search Bar (Height 3) and Tool List (Remaining)
    let left_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let search_area = left_layout[0];
    let list_area = left_layout[1];

    // 1. Search Bar
    let is_left_focused = app.focus == Focus::LeftPanel;
    let search_border_style = if is_left_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let count_text = format!("[{}/{}]", app.filtered_indices.len(), app.all_commands.len());
    let search_title = format!(" 🔍 Search (fzf) {} ", count_text);

    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(search_border_style)
        .title(Span::styled(search_title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    let search_display = if app.search_query.is_empty() {
        Line::from(vec![
            Span::styled("❯ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("Type to fuzzy search...", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("❯ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(&app.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    };

    let search_p = Paragraph::new(search_display).block(search_block);
    frame.render_widget(search_p, search_area);

    // 2. Command List
    let sort_label = format!("{} {}", app.sort_mode.label(app.language), app.sort_order.arrow());
    let lang_badge = app.language.label();
    let list_title = format!(" Tools [Sort: {}] [{}] ", sort_label, lang_badge);

    let list_border_style = if is_left_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(list_border_style)
        .title(Span::styled(list_title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(visible_idx, &actual_idx)| {
            let cmd = &app.all_commands[actual_idx];
            let is_selected = visible_idx == app.selected_index;

            let prefix = if is_selected { "❯ " } else { "  " };
            let cat_label = format!("[{}]", cmd.category(app.language));

            let line_cmd = Line::from(vec![
                Span::styled(prefix, if is_selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }),
                Span::styled(format!("{:<10}", cmd.command), if is_selected { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                Span::styled(format!(" {}", cat_label), Style::default().fg(Color::Cyan)),
            ]);

            let summary_text = cmd.summary(app.language);
            let line_desc = Line::from(vec![
                Span::raw("    "),
                Span::styled(summary_text, Style::default().fg(Color::DarkGray)),
            ]);

            let style = if is_selected {
                if is_left_focused {
                    Style::default().bg(Color::Rgb(30, 60, 90))
                } else {
                    Style::default().bg(Color::Rgb(40, 40, 40))
                }
            } else {
                Style::default()
            };

            ListItem::new(vec![line_cmd, line_desc]).style(style)
        })
        .collect();

    let list_widget = List::new(items).block(list_block);
    frame.render_widget(list_widget, list_area);
}

fn render_right_panel(app: &App, frame: &mut Frame, area: Rect) {
    let is_right_focused = app.focus == Focus::RightPanel;

    let border_style = if is_right_focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_text = if is_right_focused {
        " 📖 Detail View (ACTIVE - Scroll: ↑/↓/Page/Mouse | ESC: Back to List) "
    } else {
        " 📖 Detail Preview (Press Enter to Focus & Scroll) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(
            title_text,
            if is_right_focused {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        ));

    let selected_cmd = app.selected_command();

    let mut lines: Vec<Line> = Vec::new();

    if let Some(cmd) = selected_cmd {
        // Header line
        lines.push(Line::from(vec![
            Span::styled(&cmd.command, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("📂 {}", cmd.category(app.language)),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::raw(""));

        // Summary Section
        let sum_header = match app.language {
            Language::Japanese => "【概要 / Summary】",
            Language::English => "【Summary】",
        };
        lines.push(Line::styled(sum_header, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        lines.push(Line::styled(format!("  {}", cmd.summary(app.language)), Style::default().fg(Color::White)));
        lines.push(Line::raw(""));

        // Description Section
        let desc_text = cmd.description(app.language);
        if !desc_text.is_empty() {
            let desc_header = match app.language {
                Language::Japanese => "【詳細解説 / Description】",
                Language::English => "【Description】",
            };
            lines.push(Line::styled(desc_header, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
            lines.push(Line::styled(format!("  {}", desc_text), Style::default().fg(Color::Gray)));
            lines.push(Line::raw(""));
        }

        // Syntax Section
        if !cmd.syntax.is_empty() {
            let syn_header = match app.language {
                Language::Japanese => "【基本構文 / Syntax】",
                Language::English => "【Syntax】",
            };
            lines.push(Line::styled(syn_header, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            lines.push(Line::styled(format!("  $ {}", cmd.syntax), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
            lines.push(Line::raw(""));
        }

        // Key Options Section
        if !cmd.options.is_empty() {
            let opt_header = match app.language {
                Language::Japanese => "【主なオプション / Key Options】",
                Language::English => "【Key Options】",
            };
            lines.push(Line::styled(opt_header, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            for opt in &cmd.options {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{:<16}", opt.flag), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::raw(" : "),
                    Span::styled(opt.desc(app.language), Style::default().fg(Color::Gray)),
                ]));
            }
            lines.push(Line::raw(""));
        }

        // Practical Examples Section
        if !cmd.examples.is_empty() {
            let ex_header = match app.language {
                Language::Japanese => "【よく使う実用例 / Everyday Examples】",
                Language::English => "【Everyday Examples】",
            };
            lines.push(Line::styled(ex_header, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
            for (idx, ex) in cmd.examples.iter().enumerate() {
                lines.push(Line::styled(
                    format!("  {}. {}", idx + 1, ex.desc(app.language)),
                    Style::default().fg(Color::Cyan),
                ));
                lines.push(Line::styled(
                    format!("     ❯ {}", ex.command),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::raw(""));
            }
        }
    } else {
        lines.push(Line::styled("No matching commands found.", Style::default().fg(Color::DarkGray)));
    }

    let detail_p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    frame.render_widget(detail_p, area);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let focus_hint = if app.focus == Focus::LeftPanel {
        "[Enter] Focus Detail  "
    } else {
        "[Esc] Back to List  "
    };

    let footer_line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(focus_hint, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("[Tab] Sort  ", Style::default().fg(Color::Cyan)),
        Span::styled("[F3/Ctrl+O] Order  ", Style::default().fg(Color::Cyan)),
        Span::styled("[F2/Ctrl+L] Lang (JA/EN)  ", Style::default().fg(Color::Green)),
        Span::styled("[Esc] Clear/Quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("[Lang: {}]", app.language.label()), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    ]);

    let footer_p = Paragraph::new(footer_line).style(Style::default().bg(Color::Rgb(20, 25, 35)));
    frame.render_widget(footer_p, area);
}
