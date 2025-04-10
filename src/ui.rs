use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::block::{Position, Title},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
};

use crate::helpers;
use crate::models::{AppState, DetailsViewMode, PanelFocus, LOG_NAMES};

/// Render main app UI frame
pub fn ui(frame: &mut Frame, app_state: &mut AppState) {
    let main_layout =
        Layout::horizontal([Constraint::Max(30), Constraint::Min(0)]).split(frame.size());
    let logs_area = main_layout[0];
    let right_pane_area = main_layout[1];
    let right_layout =
        Layout::vertical([Constraint::Min(0), Constraint::Length(10)]).split(right_pane_area);
    let events_area = right_layout[0];
    let preview_area = right_layout[1];
    render_log_list(frame, app_state, logs_area);
    render_event_table(frame, app_state, events_area);
    render_preview_panel(frame, app_state, preview_area);
    
    // Render dialogs if they're visible
    render_event_details_dialog(frame, app_state);
    render_status_dialog(frame, app_state);
    render_search_bar(frame, app_state);
    render_filter_dialog(frame, app_state);
    render_help_dialog(frame, app_state);
}

fn render_log_list(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let log_items: Vec<ListItem> = LOG_NAMES.iter().map(|&name| ListItem::new(name)).collect();
    let log_list_help_line = Line::from(vec![
        Span::styled("[q]", Style::new().bold().fg(Color::Gray)),
        Span::raw(" quit "),
        Span::styled("[F1]", Style::new().bold().fg(Color::Gray)),
        Span::raw(" help"),
    ])
    .alignment(Alignment::Center);
    let log_list_help_title = Title::from(log_list_help_line)
        .position(Position::Bottom)
        .alignment(Alignment::Center);
    let log_list_block = Block::default()
        .title("Event Viewer (Local)")
        .title(log_list_help_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app_state.focus == PanelFocus::Logs {
            Color::Cyan
        } else {
            Color::White
        }));
    let log_list = List::new(log_items)
        .block(log_list_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(
            if app_state.focus == PanelFocus::Logs {
                Color::Blue
            } else {
                Color::DarkGray
            },
        ))
        .highlight_symbol("> ");
    let mut log_list_state = ListState::default();
    log_list_state.select(Some(app_state.selected_log_index));
    frame.render_stateful_widget(log_list, area, &mut log_list_state);
}

fn render_event_table(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let event_rows: Vec<Row> = app_state
        .events
        .iter()
        .map(|event| {
            let level_style = match event.level.as_str() {
                "Warning" => Style::default().fg(Color::Yellow),
                "Error" | "Critical" => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            let level_cell = Cell::from(event.level.clone()).style(level_style);
            Row::new(vec![
                level_cell,
                Cell::from(event.datetime.clone()),
                Cell::from(event.source.clone()),
                Cell::from(event.id.clone()),
            ])
        })
        .collect();
    let sort_indicator = if app_state.sort_descending {
        " ↓"
    } else {
        " ↑"
    };
    let datetime_header = format!("Date and Time{}", sort_indicator);
    let header_cells = [
        Cell::from("Level"),
        Cell::from(datetime_header),
        Cell::from("Source"),
        Cell::from("Event ID"),
    ]
    .into_iter()
    .map(|cell| {
        cell.style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray))
        .height(1);
    let widths = [
        Constraint::Length(11),
        Constraint::Length(22),
        Constraint::Percentage(60),
        Constraint::Length(10),
    ];
    let next_prev_style = if app_state.last_search_term.is_some() {
        Style::new().bold().fg(Color::Gray)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let event_table_help_line = Line::from(vec![
        Span::styled("[s]", Style::new().bold().fg(Color::Gray)),
        Span::raw(" sort "),
        Span::styled("[l]", Style::new().bold().fg(Color::Gray)),
        Span::raw(format!(
            " level ({}) ",
            app_state.get_current_level_name()
        )),
        Span::styled("[f]", Style::new().bold().fg(Color::Gray)),
        Span::raw(format!(
            " filter ({}) ",
            app_state.get_filter_status()
        )),
        Span::styled("[/]", Style::new().bold().fg(Color::Gray)),
        Span::raw(" search "),
        Span::styled("[n]", next_prev_style),
        Span::raw(" next "),
        Span::styled("[p]", next_prev_style),
        Span::raw(" prev"),
    ])
    .alignment(Alignment::Center);
    let event_table_help_title = Title::from(event_table_help_line)
        .position(Position::Bottom)
        .alignment(Alignment::Center);
    let event_table_block = Block::default()
        .title(format!("Events: {}", app_state.selected_log_name))
        .title(event_table_help_title)
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(if app_state.focus == PanelFocus::Events {
                Color::Cyan
            } else {
                Color::White
            }),
        );
    
    // Check if events list is empty, if so display message instead of table
    if app_state.events.is_empty() {
        let message = if app_state.active_filter.is_some() {
            "No events found matching filter criteria"
        } else {
            "No events found"
        };
        
        // Create a layout for vertical centering
        let inner_area = event_table_block.inner(area);
        let vertical_layout = Layout::vertical([
            Constraint::Percentage(40),  // Top space
            Constraint::Length(3),       // Message height
            Constraint::Percentage(40),  // Bottom space
        ]).split(inner_area);
        
        frame.render_widget(event_table_block, area);
        
        let centered_text = Paragraph::new(message)
            .style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
            
        frame.render_widget(centered_text, vertical_layout[1]);
    } else {
        let event_table = Table::new(event_rows, widths)
            .header(header)
            .block(event_table_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ")
            .column_spacing(1);
        frame.render_stateful_widget(event_table, area, &mut app_state.table_state);
    }
}

fn render_preview_panel(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let preview_block = Block::default()
        .title("Event Message Preview")
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(if app_state.focus == PanelFocus::Preview {
                Color::Cyan
            } else {
                Color::White
            }),
        );
    
    let preview_message = if app_state.events.is_empty() {
        if app_state.active_filter.is_some() {
            "<No events match the current filter criteria>".to_string()
        } else {
            "<No events available>".to_string()
        }
    } else if let Some(selected_index) = app_state.table_state.selected() {
        app_state
            .events
            .get(selected_index)
            .map_or("<Message not available>".to_string(), |e| e.message.clone())
    } else {
        "<No event selected>".to_string()
    };
    let message_lines = preview_message.lines().count() as u16;
    let available_height = area.height.saturating_sub(2);
    app_state.preview_scroll = app_state
        .preview_scroll
        .min(message_lines.saturating_sub(available_height));
    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        .scroll((app_state.preview_scroll, 0));
    frame.render_widget(preview_paragraph, area);
    let version_string = format!("v{}", VERSION);
    let version_width = version_string.len() as u16;
    if area.width > version_width + 2 && area.height > 1 {
        let version_x = area.right() - version_width - 1;
        let version_y = area.bottom() - 1;
        let version_rect = Rect::new(version_x, version_y, version_width, 1);
        let version_paragraph =
            Paragraph::new(version_string).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(version_paragraph, version_rect);
    }
}

fn render_event_details_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            let dialog_width = 70.min(frame.size().width.saturating_sub(4));
            let dialog_height = 20.min(frame.size().height.saturating_sub(4));
            let dialog_area = Rect::new(
                (frame.size().width - dialog_width) / 2,
                (frame.size().height - dialog_height) / 2,
                dialog_width,
                dialog_height,
            );
            frame.render_widget(Clear, dialog_area);
            let view_mode_suffix = match event_details.view_mode {
                DetailsViewMode::Formatted => " (Formatted)",
                DetailsViewMode::RawXml => " (Raw XML)",
            };
            let dialog_title = format!("{}{}", event_details.title, view_mode_suffix);
            let help_text_line = Line::from(vec![
                Span::styled(" [Esc]", Style::default().fg(Color::Gray)),
                Span::raw(" Dismiss "),
                Span::styled(" [v]", Style::default().fg(Color::Gray)),
                Span::raw(" Toggle View "),
                Span::styled(" [s]", Style::default().fg(Color::Gray)),
                Span::raw(" Save Event to Disk "),
            ])
            .alignment(Alignment::Center);
            let help_title = Title::from(help_text_line)
                .position(Position::Bottom)
                .alignment(Alignment::Center);
            let dialog_block = Block::default()
                .title(dialog_title)
                .title(help_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue));
            frame.render_widget(dialog_block.clone(), dialog_area);
            let content_area = dialog_block.inner(dialog_area);
            event_details.current_visible_height = (content_area.height as usize).max(1);
            let visible_height = event_details.current_visible_height;
            let content = event_details.current_content();
            let content_lines: Vec<&str> = content.lines().collect();
            let start_line = event_details
                .scroll_position
                .min(content_lines.len().saturating_sub(1));
            let end_line = (start_line + visible_height)
                .min(content_lines.len())
                .max(start_line);
            let visible_content = if content_lines.is_empty() {
                String::new()
            } else {
                content_lines[start_line..end_line].join("\n")
            };
            let wrap_behavior = match event_details.view_mode {
                DetailsViewMode::Formatted => Wrap { trim: true },
                DetailsViewMode::RawXml => Wrap { trim: false },
            };
            let content_paragraph = Paragraph::new(visible_content)
                .wrap(wrap_behavior)
                .style(Style::default().fg(Color::White));
            frame.render_widget(Clear, content_area);
            frame.render_widget(content_paragraph, content_area);
            if content_lines.len() > visible_height {
                let scroll_info = format!("[{}/{}]", start_line + 1, content_lines.len());
                let scroll_rect = Rect::new(
                    content_area
                        .right()
                        .saturating_sub(scroll_info.len() as u16 + 1),
                    content_area.y,
                    scroll_info.len() as u16,
                    1,
                );
                let scroll_indicator =
                    Paragraph::new(scroll_info).style(Style::default().fg(Color::Blue));
                frame.render_widget(scroll_indicator, scroll_rect);
            }
        }
    }
}

fn render_status_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(status_dialog) = &app_state.status_dialog {
        if status_dialog.visible {
            let dialog_width = 60.min(frame.size().width - 4);
            let dialog_height = 10.min(frame.size().height - 4);
            let dialog_area = Rect::new(
                (frame.size().width - dialog_width) / 2,
                (frame.size().height - dialog_height) / 2,
                dialog_width,
                dialog_height,
            );
            frame.render_widget(Clear, dialog_area);
            let border_color = if status_dialog.is_error {
                Color::Red
            } else {
                Color::Green
            };
            let dismiss_text = Line::from(vec![
                Span::styled("[Enter/Esc]", Style::default().fg(Color::White)),
                Span::raw(" Dismiss "),
            ])
            .alignment(Alignment::Center);
            let dismiss_title = Title::from(dismiss_text)
                .position(Position::Bottom)
                .alignment(Alignment::Center);
            let dialog_block = Block::default()
                .title(status_dialog.title.clone())
                .title(dismiss_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));
            frame.render_widget(dialog_block.clone(), dialog_area);
            let content_area = dialog_block.inner(dialog_area);
            let message_paragraph = Paragraph::new(status_dialog.message.clone())
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::White));
            frame.render_widget(message_paragraph, content_area);
        }
    }
}

fn render_search_bar(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_searching {
        let search_width = 40.min(frame.size().width.saturating_sub(4));
        let search_height = 3;
        let search_area = Rect::new(
            (frame.size().width - search_width) / 2,
            frame.size().height.saturating_sub(search_height + 2),
            search_width,
            search_height,
        );
        let search_block = Block::default()
            .title("Find (Enter to search, Esc to cancel)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let search_text = format!("{}_", app_state.search_term);
        let search_paragraph = Paragraph::new(search_text)
            .block(search_block.clone())
            .style(Style::default().fg(Color::White));
        frame.render_widget(Clear, search_area);
        frame.render_widget(search_paragraph, search_area);
    }
}

fn render_filter_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_filter_dialog_visible {
        let dialog_width = 60;
        let list_visible = app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Source
            && !app_state.filter_dialog_filtered_sources.is_empty();
        let list_height = if list_visible {
            5.min(app_state.filter_dialog_filtered_sources.len() as u16)
                .max(1)
        } else {
            1
        };
        let required_inner_height = 7 + list_height;
        let dialog_height = required_inner_height + 2 + 2;
        let dialog_area = helpers::centered_fixed_rect(
            dialog_width,
            dialog_height.min(frame.size().height),
            frame.size(),
        );
        frame.render_widget(Clear, dialog_area);
        let esc_hint_line = Line::from(vec![
            Span::styled("[Esc]", Style::new().bold().fg(Color::Gray)),
            Span::raw(" Cancel"),
        ])
        .alignment(Alignment::Center);
        let esc_hint_title = Title::from(esc_hint_line)
            .position(Position::Bottom)
            .alignment(Alignment::Center);
        let dialog_block = Block::default()
            .title("Filter Events")
            .title(esc_hint_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));
        let inner_area = dialog_block.inner(dialog_area);
        frame.render_widget(dialog_block.clone(), dialog_area);
        let source_area_height = 1 + 1 + list_height;
        let constraints = vec![
            Constraint::Length(source_area_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(constraints)
            .split(inner_area);
        let source_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(chunks[0]);
        let focused_style = Style::default().bg(Color::DarkGray);
        let unfocused_style = Style::default();
        frame.render_widget(Paragraph::new("Source:"), source_chunks[0]);
        let source_style = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Source {
            focused_style
        } else {
            unfocused_style
        };
        let source_input_display = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Source {
            format!("{}_", app_state.filter_dialog_source_input)
        } else if app_state.filter_dialog_source_input.is_empty() {
            "[Any Source]".to_string()
        } else {
            app_state.filter_dialog_source_input.clone()
        };
        frame.render_widget(
            Paragraph::new(source_input_display).style(source_style),
            source_chunks[1],
        );
        if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Source
            && !app_state.filter_dialog_filtered_sources.is_empty()
        {
            let list_items: Vec<ListItem> = app_state
                .filter_dialog_filtered_sources
                .iter()
                .map(|(_, name)| ListItem::new(name.clone()))
                .collect();
            let list = List::new(list_items)
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Blue),
                )
                .highlight_symbol("> ");
            let mut list_state = ListState::default();
            list_state.select(app_state.filter_dialog_filtered_source_selection);
            frame.render_stateful_widget(list, source_chunks[3], &mut list_state);
        }
        frame.render_widget(Paragraph::new("Event ID:"), chunks[1]);
        let event_id_input_style = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::EventId {
            focused_style
        } else {
            unfocused_style
        };
        let event_id_text = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::EventId {
            format!("{}_", app_state.filter_dialog_event_id)
        } else {
            app_state.filter_dialog_event_id.clone()
        };
        frame.render_widget(
            Paragraph::new(event_id_text).style(event_id_input_style),
            chunks[2],
        );
        let level_text = Line::from(vec![
            Span::raw("Level: "),
            Span::styled("< ", Style::default().fg(Color::Yellow)),
            Span::styled(
                app_state.filter_dialog_level.display_name(),
                if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Level {
                    focused_style.add_modifier(Modifier::BOLD)
                } else {
                    unfocused_style
                },
            ),
            Span::styled(" >", Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(level_text), chunks[3]);
        let apply_style = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Apply {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let clear_style = if app_state.filter_dialog_focus == crate::models::FilterFieldFocus::Clear {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let apply_text = Span::styled(" [ Apply ] ", apply_style);
        let clear_text = Span::styled(" [ Clear ] ", clear_style);
        frame.render_widget(Paragraph::new(""), chunks[4]);
        let button_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[5]);
        frame.render_widget(
            Paragraph::new(apply_text).alignment(Alignment::Center),
            button_layout[0],
        );
        frame.render_widget(
            Paragraph::new(clear_text).alignment(Alignment::Center),
            button_layout[1],
        );
    }
}

fn render_help_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.help_dialog_visible {
        let help_width = 80.min(frame.size().width.saturating_sub(4));
        let help_height = 30.min(frame.size().height.saturating_sub(4));
        let help_area = helpers::centered_fixed_rect(help_width, help_height, frame.size());
        frame.render_widget(Clear, help_area);
        let dismiss_text = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Gray).bold()),
            Span::raw(" Dismiss "),
            Span::styled(
                " ↑↓ PgUp/Dn Home/End ",
                Style::default().fg(Color::Gray).bold(),
            ),
            Span::raw(" Scroll "),
        ])
        .alignment(Alignment::Center);
        let dismiss_title = Title::from(dismiss_text)
            .position(Position::Bottom)
            .alignment(Alignment::Center);
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        let help_dialog_title = format!(" Help - Event Commander (v{}) ", VERSION);
        let help_block = Block::default()
            .title(help_dialog_title)
            .title(dismiss_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let content_area = help_block.inner(help_area);
        frame.render_widget(help_block, help_area);
        let help_text = vec![
            Line::from(Span::styled(
                "Event Commander",
                Style::default().bold().fg(Color::Cyan),
            )),
            Line::from("A simple TUI for browsing Windows Event Logs."),
            Line::from(""),
            Line::from(vec![
                Span::raw("Developed by: "),
                Span::styled("Toby Martin", Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Source Code: "),
                Span::styled(
                    "https://github.com/Dastari/event_commander",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "License: GPL-3.0-or-later",
                Style::default().fg(Color::Magenta),
            )),
            Line::from("  This program is free software: you can redistribute it and/or modify"),
            Line::from("  it under the terms of the GNU General Public License as published by"),
            Line::from("  the Free Software Foundation, either version 3 of the License, or"),
            Line::from("  (at your option) any later version. See LICENSE.txt for details."),
            Line::from(""),
            Line::from(Span::styled(
                "--- Keybindings ---",
                Style::default().bold().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled("Global:", Style::default().underlined())),
            Line::from(vec![
                Span::styled("  q       ", Style::default().bold()),
                Span::raw("Quit application"),
            ]),
            Line::from(vec![
                Span::styled("  F1      ", Style::default().bold()),
                Span::raw("Show this help screen"),
            ]),
            Line::from(vec![
                Span::styled("  Tab     ", Style::default().bold()),
                Span::raw("Cycle focus forward (Logs -> Events -> Preview)"),
            ]),
            Line::from(vec![
                Span::styled("  S-Tab   ", Style::default().bold()),
                Span::raw("Cycle focus backward"),
            ]),
            // Rest of the help text omitted for brevity
        ];
        let total_lines = help_text.len();
        let visible_height = content_area.height as usize;
        let max_scroll = total_lines.saturating_sub(visible_height);
        app_state.help_scroll_position = app_state.help_scroll_position.min(max_scroll);
        let current_scroll = app_state.help_scroll_position;
        let help_paragraph = Paragraph::new(help_text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White))
            .scroll((current_scroll as u16, 0));
        frame.render_widget(help_paragraph, content_area);
        if total_lines > visible_height {
            let scroll_info = format!("[{}/{}]", current_scroll + 1, total_lines);
            let scroll_rect = Rect::new(
                content_area
                    .right()
                    .saturating_sub(scroll_info.len() as u16 + 1),
                content_area.y,
                scroll_info.len() as u16,
                1,
            );
            let scroll_indicator =
                Paragraph::new(scroll_info).style(Style::default().fg(Color::Yellow));
            frame.render_widget(scroll_indicator, scroll_rect);
        }
    }
} 