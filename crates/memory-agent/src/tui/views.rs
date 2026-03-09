use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use super::{App, InputMode, TreeRow, View};

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let has_update = app.update_notice.is_some();

    // Optional update banner + tabs + main content + help
    let constraints = if has_update {
        vec![
            Constraint::Length(1), // update banner
            Constraint::Length(1), // tabs
            Constraint::Fill(1),   // content
            Constraint::Length(1), // help
        ]
    } else {
        vec![
            Constraint::Length(1), // tabs
            Constraint::Fill(1),   // content
            Constraint::Length(1), // help
        ]
    };

    let areas = Layout::vertical(constraints).split(area);

    let (tabs_area, main_area, help_area) = if has_update {
        render_update_banner(frame, app, areas[0]);
        (areas[1], areas[2], areas[3])
    } else {
        (areas[0], areas[1], areas[2])
    };

    render_tabs(frame, app, tabs_area);
    render_help(frame, app, help_area);

    match &app.view.clone() {
        View::Search => render_search(frame, app, main_area),
        View::Detail(_) => render_detail(frame, app, main_area),
        View::Live => render_live(frame, app, main_area),
        View::Metrics => render_metrics(frame, app, main_area),
        View::ScopeTree => render_scope_tree(frame, app, main_area),
        View::HookConfig => render_hook_config(frame, app, main_area),
    }

    // Delete confirmation overlay
    if let Some((id, ref key)) = app.confirm_delete {
        render_confirm_delete(frame, id, key, area);
    }

    // Inline template editor overlay
    if let Some((_, ref ta)) = app.template_editor {
        let width = area.width.saturating_sub(4).min(100);
        let height = area.height.saturating_sub(4).min(30);
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = ratatui::layout::Rect::new(x, y, width, height);
        frame.render_widget(ratatui::widgets::Clear, popup);
        frame.render_widget(ta, popup);
    }
}

fn render_update_banner(frame: &mut Frame, app: &App, area: Rect) {
    let msg = app.update_notice.as_deref().unwrap_or("");
    let banner = Paragraph::new(Line::from(vec![
        Span::styled(
            " ▲ ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {msg} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(banner, area);
}

fn render_confirm_delete(frame: &mut Frame, id: i64, key: &str, area: Rect) {
    let width = 50.min(area.width.saturating_sub(4));
    let height = 5;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let text = vec![
        Line::from(vec![
            Span::raw("Delete memory "),
            Span::styled(format!("#{id}"), Style::default().fg(Color::Yellow)),
            Span::raw(format!(" ({})?", truncate(key, 20))),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("=soft delete  "),
            Span::styled(
                "Y",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("=hard delete  "),
            Span::raw("any other=cancel"),
        ]),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" Confirm Delete ");
    frame.render_widget(Paragraph::new(text).block(block), popup);
}

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs = vec![
        (
            " Search ",
            matches!(app.view, View::Search | View::Detail(_)),
        ),
        (" Live ", matches!(app.view, View::Live)),
        (" Metrics ", matches!(app.view, View::Metrics)),
        (" Scopes ", matches!(app.view, View::ScopeTree)),
        (" Hook Config ", matches!(app.view, View::HookConfig)),
    ];

    let spans: Vec<Span> = tabs
        .into_iter()
        .map(|(label, active)| {
            if active {
                Span::styled(
                    label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(label, Style::default().fg(Color::DarkGray))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let help = if app.template_editor.is_some() {
        " Ctrl+S=save  Esc=cancel  (arrow keys to move, type to edit)"
    } else {
        match app.input_mode {
            InputMode::Search => " Enter=search  Esc=cancel  Backspace=delete",
            InputMode::Normal => match &app.view {
                View::Search => {
                    " /=search  Enter=detail  d=delete  r=refresh  j/k=scroll  Tab=next  q=quit"
                }
                View::Detail(_) => " d=delete  r=refresh  j/k=scroll  Esc=back  Tab=next  q=quit",
                View::ScopeTree => {
                    " Enter=expand/detail  d=delete  r=refresh  j/k=scroll  Tab=next  q=quit"
                }
                View::HookConfig => " e=edit  x=clear  Tab=next  q=quit",
                View::Live => " r=refresh  j/k=scroll  Tab=next  q=quit",
                _ => " r=refresh  j/k=scroll  Tab=next  q=quit",
            },
        }
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_search(frame: &mut Frame, app: &mut App, area: Rect) {
    let [input_area, results_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    // Search input
    let input_style = if app.input_mode == InputMode::Search {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("Search (press / to type)")
        .border_style(input_style);
    let input_widget = Paragraph::new(app.search_input.as_str()).block(input_block);
    frame.render_widget(input_widget, input_area);

    // Show cursor when in search mode
    if app.input_mode == InputMode::Search {
        let cursor_x = (input_area.x + 1 + app.search_input.len() as u16)
            .min(input_area.x + input_area.width.saturating_sub(2));
        frame.set_cursor_position((cursor_x, input_area.y + 1));
    }

    // Results list
    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|r| {
            let line = Line::from(vec![
                Span::styled(format!("{:<6}", r.id), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<30}", truncate(&r.key, 28)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!(" {:<14}", truncate(&r.scope, 12)),
                    Style::default().fg(Color::Blue),
                ),
                Span::styled(
                    format!(" {:.2} ", r.confidence),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(truncate(&r.value_preview, 40)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let count = app.search_results.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Memories ({count})")),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, results_area, &mut app.list_state);
}

fn render_detail(frame: &mut Frame, app: &App, area: Rect) {
    let memory = match &app.detail_memory {
        Some(m) => m,
        None => return,
    };

    let [content_area, meta_area] = Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1)])
        .direction(Direction::Horizontal)
        .areas(area);

    // Content panel
    let content = Paragraph::new(memory.value.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("  {}  ", memory.key)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    frame.render_widget(content, content_area);

    // Metadata panel
    let mut lines = vec![
        Line::from(vec![
            Span::styled("ID:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.id.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Scope:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(memory.scope.clone(), Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("Source:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.source_type.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Conf:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}", memory.confidence),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Revs:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.revision_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Dups:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.duplicate_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate(&memory.created_at, 19)),
        ]),
        Line::from(vec![
            Span::styled("Accessed:", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate(&memory.accessed_at, 19)),
        ]),
    ];

    if let Some(ref sr) = memory.source_ref {
        lines.push(Line::from(vec![
            Span::styled("Ref:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate(sr, 24)),
        ]));
    }

    if let Some(ref tags) = memory.tags {
        if !tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Tags:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(tags.join(", "), Style::default().fg(Color::Magenta)),
            ]));
        }
    }

    let meta =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Metadata"));
    frame.render_widget(meta, meta_area);
}

fn render_live(frame: &mut Frame, app: &mut App, area: Rect) {
    let (saves, injections, searches, tokens) = app.live_summary;

    // Header bar with today's summary
    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    let header_line = Line::from(vec![
        Span::styled(" Today: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{saves} saves"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{injections} injections"),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{searches} searches"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{tokens} tok"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  (auto-refresh 2s)", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(header_line).block(Block::default().borders(Borders::ALL).title(" Live ")),
        header_area,
    );

    // Event list — newest first (already ordered by store)
    let items: Vec<ListItem> = app
        .live_events
        .iter()
        .map(|e| {
            let time = e.created_at.get(11..19).unwrap_or("?");
            let (action_color, action_label) = match e.action.as_str() {
                "save" => (Color::Green, "SAVE  "),
                "inject" | "inject_budget" | "inject_context" | "auto-context" => {
                    (Color::Blue, "INJECT")
                }
                "search" | "detail" | "list" | "context" | "budget" => (Color::Yellow, "SEARCH"),
                _ => (Color::DarkGray, "OTHER "),
            };

            let tok_str = if e.tokens > 0 {
                format!("  {} tok", e.tokens)
            } else {
                String::new()
            };

            let key_str = if e.key.is_empty() {
                String::new()
            } else {
                format!("  {}", truncate(&e.key, 30))
            };

            let scope_str = if e.scope == "/" {
                String::new()
            } else {
                format!("  [{}]", truncate(&e.scope, 18))
            };

            let line = Line::from(vec![
                Span::styled(format!("{time}  "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    action_label,
                    Style::default()
                        .fg(action_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(key_str, Style::default().fg(Color::Cyan)),
                Span::styled(scope_str, Style::default().fg(Color::Blue)),
                Span::styled(tok_str, Style::default().fg(Color::Green)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let count = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Recent events ({count}, newest first)")),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, &mut app.live_state);
}

fn render_metrics(frame: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Key").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Injections").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Hits").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Hit Rate").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .metrics
        .iter()
        .map(|m| {
            let rate_style = if m.hit_rate >= 0.5 {
                Style::default().fg(Color::Green)
            } else if m.hit_rate >= 0.2 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Red)
            };
            Row::new(vec![
                Cell::from(m.id.to_string()),
                Cell::from(truncate(&m.key, 30)),
                Cell::from(m.injections.to_string()),
                Cell::from(m.hits.to_string()),
                Cell::from(format!("{:.1}%", m.hit_rate * 100.0)).style(rate_style),
            ])
        })
        .collect();

    let count = rows.len();
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Fill(1),
            Constraint::Length(11),
            Constraint::Length(6),
            Constraint::Length(9),
        ],
    )
    .header(header.style(Style::default().fg(Color::Yellow)))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Metrics ({count}) — sorted by injections")),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_scope_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .tree_rows
        .iter()
        .map(|row| match row {
            TreeRow::Scope { path, count } => {
                let expanded = app.expanded_scopes.contains(path);
                let arrow = if expanded { "▼" } else { "▶" };
                let depth = path.matches('/').count().saturating_sub(1);
                let indent = "  ".repeat(depth);
                let name = path.rsplit('/').next().unwrap_or(path.as_str());
                let display_name = if name.is_empty() { "/" } else { name };
                let line = Line::from(vec![
                    Span::raw(format!("{indent}{arrow} ")),
                    Span::styled(
                        display_name.to_string(),
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("  ({count})"), Style::default().fg(Color::DarkGray)),
                ]);
                ListItem::new(line)
            }
            TreeRow::Memory { id, key, preview } => {
                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("{:<5}", id), Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:<28}", truncate(key, 26)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(truncate(preview, 40)),
                ]);
                ListItem::new(line)
            }
        })
        .collect();

    let total_scopes = app
        .tree_rows
        .iter()
        .filter(|r| matches!(r, TreeRow::Scope { .. }))
        .count();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Scopes ({total_scopes}) — Enter to expand/collapse"
        )))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.tree_state);
}

fn render_hook_config(frame: &mut Frame, app: &App, area: Rect) {
    let is_set = app.hooks_config.injection_prompt.is_some();
    let preview = app
        .hooks_config
        .injection_prompt
        .as_deref()
        .map(|t| format!("[set] {}", truncate(&t.replace('\n', " "), 55)))
        .unwrap_or_else(|| "[empty]".to_string());
    let hint = if is_set {
        "  ← e=edit  x=clear"
    } else {
        "  ← press e to set"
    };

    let row = Line::from(vec![
        Span::styled("injection_prompt  ", Style::default().fg(Color::Cyan)),
        Span::raw(preview),
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ]);

    let mut all_rows = vec![row];
    if let Some(ref msg) = app.hook_status {
        all_rows.push(Line::raw(""));
        let status_color = if msg.starts_with("Error") {
            Color::Red
        } else if msg.starts_with("Pending") {
            Color::Yellow
        } else if msg.starts_with("Discarded") || msg.starts_with("No changes") {
            Color::DarkGray
        } else {
            Color::Green
        };
        all_rows.push(Line::from(Span::styled(
            msg.as_str(),
            Style::default().fg(status_color),
        )));
    }

    frame.render_widget(
        Paragraph::new(all_rows)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Hook Config — static injection_prompt "),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", memory_core::safe_truncate(s, max.saturating_sub(1)))
    } else {
        s.to_string()
    }
}
