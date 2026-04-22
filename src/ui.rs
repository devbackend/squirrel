use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    app::Screen,
    models::{QueryResult, FORM_FIELD_NAMES},
};

pub fn render(f: &mut Frame, screen: &Screen, status: Option<&str>) {
    let area = f.area();

    // Split into main area + status bar at the bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let main_area = chunks[0];
    let status_area = chunks[1];

    match screen {
        Screen::ConnectionList { connections, selected } => {
            render_list(f, main_area, " Squirrel — Connections ", connections, *selected);
            render_hint(f, status_area, "n: new  r: rename  e: edit  d: delete  Enter: open  q: quit", status);
        }
        Screen::CreateConnection { form, status: form_status } => {
            render_connection_form(f, main_area, form_status.as_deref());
            render_form_fields(f, main_area, &form.values, form.active_field, form.editing);
            let hint = if form.editing {
                "Type text  Enter/Esc: done"
            } else {
                "j/k: move  Enter: edit field  s: save & test  Esc: back"
            };
            render_hint(f, status_area, hint, status);
        }
        Screen::QueryList { connection, queries, selected, preview } => {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(main_area);

            let title = format!(" Queries — {connection} ");
            render_list(f, panes[0], &title, queries, *selected);

            let preview_title = queries.get(*selected)
                .map(|q| format!(" {q} "))
                .unwrap_or_else(|| " Preview ".to_string());
            let preview_block = Paragraph::new(preview.as_str())
                .block(Block::default().borders(Borders::ALL).title(preview_title))
                .wrap(Wrap { trim: false });
            f.render_widget(preview_block, panes[1]);

            render_hint(f, status_area, "n: new  r: rename  e: edit  d: delete  Enter: run  ←/Esc: back", status);
        }
        Screen::CreateQueryName { connection, input } => {
            render_name_input(f, main_area, connection, input);
            render_hint(f, status_area, "Enter: open editor  Esc: back", status);
        }
        Screen::RenameConnection { old_name, input } => {
            render_rename_input(f, main_area, old_name, input);
            render_hint(f, status_area, "Enter: confirm  Esc: cancel", status);
        }
        Screen::RenameQuery { old_name, input, .. } => {
            render_rename_input(f, main_area, old_name, input);
            render_hint(f, status_area, "Enter: confirm  Esc: cancel", status);
        }
        Screen::Results { connection, query, result } => {
            render_results(f, main_area, connection, query, result);
            render_hint(f, status_area, "j/k: row  ←h/→l: page  q/Esc: back", status);
        }
    }
}

// ── List ───────────────────────────────────────────────────────────────────────

fn render_list(f: &mut Frame, area: Rect, title: &str, items: &[String], selected: usize) {
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "▶ " } else { "  " };
            ListItem::new(format!("{prefix}{name}")).style(style)
        })
        .collect();

    let hint = if items.is_empty() {
        " (empty — press n to create) "
    } else {
        ""
    };
    let full_title = format!("{title}{hint}");

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(full_title));
    f.render_widget(list, area);
}

// ── Connection form ────────────────────────────────────────────────────────────

fn render_connection_form(f: &mut Frame, area: Rect, _status: Option<&str>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" New Connection ");
    f.render_widget(block, area);
}

fn render_form_fields(f: &mut Frame, area: Rect, values: &[String; 6], active: usize, editing: bool) {
    // Leave a 1-cell border padding
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Each field takes 3 rows: label, input box (1 row), blank
    let constraints: Vec<Constraint> = FORM_FIELD_NAMES
        .iter()
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(3), Constraint::Length(1)])
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, name) in FORM_FIELD_NAMES.iter().enumerate() {
        let label_area = chunks[i * 3];
        let input_area = chunks[i * 3 + 1];

        let label_style = if i == active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let label = Paragraph::new(Span::styled(*name, label_style));
        f.render_widget(label, label_area);

        let is_editing = i == active && editing;

        let display_value = if i == 5 {
            let masked = "*".repeat(values[i].len());
            if is_editing { format!("{masked}_") } else { masked }
        } else if is_editing {
            format!("{}_", values[i])
        } else {
            values[i].clone()
        };

        let input_style = if i == active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let border_style = if is_editing {
            Style::default().fg(Color::Cyan)
        } else if i == active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let input = Paragraph::new(display_value.as_str())
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).border_style(border_style));
        f.render_widget(input, input_area);
    }
}

// ── Query name input ───────────────────────────────────────────────────────────

fn render_name_input(f: &mut Frame, area: Rect, connection: &str, input: &str) {
    // Center a small dialog
    let dialog = centered_rect(50, 20, area);

    f.render_widget(Clear, dialog);

    let title = format!(" New query for '{connection}' ");
    let content = format!("Name: {input}_");
    let paragraph = Paragraph::new(content.as_str())
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));
    f.render_widget(paragraph, dialog);
}

fn render_rename_input(f: &mut Frame, area: Rect, old_name: &str, input: &str) {
    let dialog = centered_rect(50, 20, area);
    f.render_widget(Clear, dialog);
    let title = format!(" Rename '{old_name}' ");
    let content = format!("New name: {input}_");
    let paragraph = Paragraph::new(content.as_str())
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));
    f.render_widget(paragraph, dialog);
}

// ── Results ────────────────────────────────────────────────────────────────────

/// Alternating dark background for zebra-striped even rows.
const ZEBRA_BG: Color = Color::Rgb(30, 30, 40);

fn render_results(
    f: &mut Frame,
    area: Rect,
    connection: &str,
    query: &str,
    result: &QueryResult,
) {
    match result {
        QueryResult::AffectedRows(n) => {
            let title = format!(" {connection} › {query} — Result ");
            let msg = format!("{n} row(s) affected");
            let paragraph = Paragraph::new(msg)
                .block(Block::default().borders(Borders::ALL).title(title))
                .style(Style::default().fg(Color::Green));
            f.render_widget(paragraph, area);
        }
        QueryResult::Rows { columns, rows, page, page_size: _, selected_row } => {
            let page_count = result.page_count();
            let title = format!(
                " {connection} › {query} — {} rows (page {}/{page_count}) ",
                rows.len(),
                page + 1,
            );

            if columns.is_empty() {
                let paragraph = Paragraph::new("(no rows returned)")
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(paragraph, area);
                return;
            }

            let page_rows = result.current_page_rows();

            // Compute column widths: max of header and data, capped at 30.
            let col_widths: Vec<usize> = columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let data_max = page_rows.iter().map(|r| r[i].len()).max().unwrap_or(0);
                    col.len().max(data_max).clamp(4, 30)
                })
                .collect();

            let constraints: Vec<Constraint> =
                col_widths.iter().map(|w| Constraint::Min(*w as u16)).collect();

            // Header row — bold yellow, with a blank margin row beneath it.
            let header = Row::new(columns.iter().map(|c| {
                Cell::from(c.as_str())
                    .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))
            }))
            .bottom_margin(1);

            // Data rows with zebra striping.  Even rows (0-indexed) get a
            // subtle dark background; NULL values are rendered in dim grey.
            let table_rows: Vec<Row> = page_rows
                .iter()
                .enumerate()
                .map(|(row_idx, row)| {
                    let row_bg = if row_idx % 2 == 1 {
                        Style::default().bg(ZEBRA_BG)
                    } else {
                        Style::default()
                    };
                    Row::new(row.iter().map(|cell| {
                        let cell_style = if cell == "NULL" {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default()
                        };
                        Cell::from(cell.as_str()).style(cell_style)
                    }))
                    .style(row_bg)
                })
                .collect();

            let table = Table::new(table_rows, constraints)
                .header(header)
                .block(Block::default().borders(Borders::ALL).title(title))
                // Highlight style for the selected row: cyan foreground, bold.
                .row_highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                // A leading marker so the selected row is visually distinct
                // even on terminals where colour support is limited.
                .highlight_symbol("▶ ");

            let mut state = TableState::default();
            state.select(Some(*selected_row));

            f.render_stateful_widget(table, area, &mut state);
        }
    }
}

// ── Status / hint bar ──────────────────────────────────────────────────────────

fn render_hint(f: &mut Frame, area: Rect, hint: &str, status: Option<&str>) {
    let line = if let Some(msg) = status {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(msg, Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![Span::styled(
            format!(" {hint}"),
            Style::default().fg(Color::DarkGray),
        )])
    };
    f.render_widget(Paragraph::new(line), area);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
