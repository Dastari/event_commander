use lazy_static::lazy_static;
use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::block::{Position, Title},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        Wrap, BorderType,
    },
};

use crate::helpers;
use crate::models::{AppState, FilterFieldFocus, PanelFocus, LOG_NAMES, PreviewViewMode};

// --- Theme Constants ---
const THEME_BG: Color = Color::Blue;
const THEME_FG: Color = Color::White;
const THEME_BORDER: Color = Color::LightCyan;
const THEME_HIGHLIGHT_BG: Color = Color::Cyan;
const THEME_HIGHLIGHT_FG: Color = THEME_BG;
const THEME_ALT_FG: Color = Color::LightYellow;
const THEME_ERROR_FG: Color = Color::LightRed;
const THEME_WARN_FG: Color = Color::LightYellow;
const THEME_DIALOG_DEFAULT_BG: Color = Color::Cyan;
const THEME_DIALOG_DEFAULT_FG: Color = Color::Black;
const THEME_DIALOG_ERROR_BG: Color = Color::Red;
const THEME_DIALOG_ERROR_FG: Color = Color::LightYellow;
const THEME_DIALOG_WARN_BG: Color = Color::Yellow;
const THEME_DIALOG_WARN_FG: Color = Color::LightYellow;
const THEME_FOOTER_BG: Color = Color::Black;
const THEME_FOOTER_FG: Color = Color::Gray;
const BORDER_TYPE_THEME: BorderType = BorderType::Double;
const VERSION: &str = env!("CARGO_PKG_VERSION");

const WHITE: Color = Color::White;
const GRAY: Color = Color::Gray;
const DARK_GRAY: Color = Color::DarkGray;
const RED: Color = Color::Red;
const GREEN: Color = Color::Green;
const MAGENTA: Color = Color::Magenta;

lazy_static! {
    // Core Theme Styles
    static ref DEFAULT_STYLE: Style = Style::new().bg(THEME_BG).fg(THEME_FG);
    static ref BORDER_STYLE: Style = Style::new().fg(THEME_BORDER);
    static ref SELECTION_STYLE: Style = Style::new().bg(THEME_HIGHLIGHT_BG).fg(THEME_HIGHLIGHT_FG);
    static ref ALT_FG_STYLE: Style = DEFAULT_STYLE.patch(Style::new().fg(THEME_ALT_FG));
    static ref ERROR_FG_STYLE: Style = DEFAULT_STYLE.patch(Style::new().fg(THEME_ERROR_FG));
    static ref WARN_FG_STYLE: Style = DEFAULT_STYLE.patch(Style::new().fg(THEME_WARN_FG));
    static ref TITLE_STYLE: Style = *SELECTION_STYLE;
    static ref FOOTER_STYLE: Style = Style::new().bg(THEME_FOOTER_BG).fg(THEME_FOOTER_FG);
    static ref DIALOG_SELECTION_STYLE: Style = Style::new().bg(THEME_DIALOG_DEFAULT_FG).fg(THEME_ALT_FG);
    static ref DIALOG_DEFAULT_STYLE: Style = Style::new().bg(THEME_DIALOG_DEFAULT_BG).fg(THEME_DIALOG_DEFAULT_FG);
    static ref DIALOG_ERROR_STYLE: Style = Style::new().bg(THEME_DIALOG_ERROR_BG).fg(THEME_DIALOG_ERROR_FG);
    static ref DIALOG_WARN_STYLE: Style = Style::new().bg(THEME_DIALOG_WARN_BG).fg(THEME_DIALOG_WARN_FG);

    // Component Styles
    static ref BOLD_STYLE: Style = DEFAULT_STYLE.patch(Style::new().add_modifier(Modifier::BOLD));
    static ref HEADER_STYLE: Style = DEFAULT_STYLE.patch(Style::new().fg(THEME_ALT_FG).add_modifier(Modifier::BOLD));
    static ref HEADER_ROW_STYLE: Style = *DEFAULT_STYLE;
    static ref INPUT_FOCUSED_STYLE: Style = *SELECTION_STYLE;
    static ref INPUT_UNFOCUSED_STYLE: Style = *DEFAULT_STYLE;

    // Keybinding Styles
    static ref KEY_STYLE: Style = *SELECTION_STYLE;
    static ref KEY_Q: Span<'static> = Span::styled("[q]", *KEY_STYLE);
    static ref KEY_F1: Span<'static> = Span::styled("[F1]", *KEY_STYLE);
    static ref KEY_S_SORT: Span<'static> = Span::styled("[s]", *KEY_STYLE);
    static ref KEY_L_LEVEL: Span<'static> = Span::styled("[l]", *KEY_STYLE);
    static ref KEY_F_FILTER: Span<'static> = Span::styled("[f]", *KEY_STYLE);
    static ref KEY_SLASH_SEARCH: Span<'static> = Span::styled("[/]", *KEY_STYLE);
    static ref KEY_N_NEXT: Span<'static> = Span::styled("[n]", *KEY_STYLE);
    static ref KEY_P_PREV: Span<'static> = Span::styled("[p]", *KEY_STYLE);
    static ref KEY_ESC: Span<'static> = Span::styled("[Esc]", *KEY_STYLE);
    static ref KEY_ESC_LEFT: Span<'static> = Span::styled("[Esc/←]", *KEY_STYLE);
    static ref KEY_V_TOGGLE: Span<'static> = Span::styled("[v]", *KEY_STYLE);
    static ref KEY_S_SAVE: Span<'static> = Span::styled("[s]", *KEY_STYLE);
    static ref KEY_ENTER_ESC: Span<'static> = Span::styled("[Enter/Esc]", *KEY_STYLE);
    static ref KEY_SCROLL: Span<'static> = Span::styled("[↑↓ PgUpDn HmEnd]", *KEY_STYLE);

    // Static Titles/Lines

    static ref STATUS_DISMISS_LINE: Line<'static> = Line::from(vec![
        KEY_ENTER_ESC.clone(), Span::raw(" Dismiss "),
    ]).alignment(Alignment::Center);
    static ref STATUS_DISMISS_TITLE: Title<'static> = Title::from(STATUS_DISMISS_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref FILTER_CANCEL_LINE: Line<'static> = Line::from(vec![
        KEY_ESC.clone(), Span::raw(" Cancel"),
    ]).alignment(Alignment::Center);
    static ref FILTER_CANCEL_TITLE: Title<'static> = Title::from(FILTER_CANCEL_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref SEARCH_BAR_TITLE: Title<'static> = Title::from(
        Span::styled(" Find (Enter to search, Esc to cancel) ", *TITLE_STYLE)
    ).alignment(Alignment::Left).position(Position::Top);

    static ref HELP_DISMISS_TEXT_LINE: Line<'static> = Line::from(vec![
        KEY_ESC.clone(),
        Span::raw(" Dismiss "),
        KEY_SCROLL.clone(),
        Span::raw(" Scroll "),
    ]).alignment(Alignment::Center);
    static ref HELP_DISMISS_TITLE: Title<'static> = Title::from(HELP_DISMISS_TEXT_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref HELP_TEXT_LINES: Vec<Line<'static>> = vec![
        Line::from(Span::styled("Event Commander", BOLD_STYLE.patch(Style::new().fg(THEME_FG)))),
        Line::from(Span::raw("A simple TUI for browsing Windows Event Logs.")),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::raw("Developed by: "),
            Span::styled("Toby Martin", Style::new().fg(GREEN)),
        ]),
        Line::from(vec![
            Span::raw("Source Code: "),
            Span::styled(
                "https://github.com/Dastari/event_commander",
                DEFAULT_STYLE.patch(Style::new().fg(THEME_FG).add_modifier(Modifier::UNDERLINED)),
            ),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled("License: GPL-3.0-or-later", Style::new().fg(MAGENTA))),
        Line::from(Span::raw("  This program is free software...")),
        Line::from(Span::raw("  it under the terms...")),
        Line::from(Span::raw("  the Free Software Foundation...")),
        Line::from(Span::raw("  (at your option) any later version...")),
        Line::from(Span::raw("")),
        Line::from(Span::styled("--- Keybindings ---", ALT_FG_STYLE.patch(Style::new().add_modifier(Modifier::BOLD)))),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  [q]      ", *KEY_STYLE),
            Span::raw("Quit application"),
        ]),
        // Add more keybindings as needed...
    ];
}

// --- Helper Functions ---

fn create_dialog_block(title_text: &str, bottom_title: Title<'static>, dialog_style: Style) -> Block<'static> {
    let top_title_style = dialog_style.patch(Style::new().add_modifier(Modifier::BOLD));
    let title = Title::from(Span::styled(format!(" {} ", title_text), top_title_style))
        .alignment(Alignment::Left)
        .position(Position::Top);
    Block::new()
        .title(title)
        .title(bottom_title)
        .borders(Borders::ALL)
        .border_style(dialog_style)
        .border_type(BORDER_TYPE_THEME)
        .style(dialog_style)
}

fn render_scroll_indicator(
    frame: &mut Frame,
    area: Rect,
    current_line: usize,
    total_lines: usize,
    style: Style,
) {
    if total_lines > area.height as usize && area.width > 5 {
        let scroll_info = format!("[{}/{}]", current_line, total_lines);
        let indicator_width = scroll_info.len() as u16;
        let x_pos = area.right().saturating_sub(indicator_width);
        let y_pos = area.y;
        let scroll_rect = Rect::new(x_pos, y_pos, indicator_width, 1);
        frame.render_widget(Paragraph::new(scroll_info).style(style), scroll_rect);
    }
}

// --- Main UI Rendering ---

pub fn ui(frame: &mut Frame, app_state: &mut AppState) {
    let main_chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.size());

    render_log_tabs(frame, app_state, main_chunks[0]);
    let middle_chunks = Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(main_chunks[1]);
    render_event_table(frame, app_state, middle_chunks[0]);
    render_preview_panel(frame, app_state, middle_chunks[1]);
    render_bottom_bar(frame, app_state, main_chunks[2]);

    render_status_dialog(frame, app_state);
    render_filter_dialog(frame, app_state);
    render_help_dialog(frame, app_state);
    render_search_bar(frame, app_state);
}

// --- Panel Rendering ---

fn render_log_tabs(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let block = Block::new()
        .title(Title::from(Span::styled(" Event Commander ", *TITLE_STYLE)).alignment(Alignment::Left).position(Position::Top))
        .title(Title::from(Span::styled(format!("v{}", VERSION), *DEFAULT_STYLE)).alignment(Alignment::Right).position(Position::Top))
        .borders(Borders::ALL)
        .border_style(*BORDER_STYLE)
        .border_type(BORDER_TYPE_THEME)
        .style(*DEFAULT_STYLE);
    frame.render_widget(block.clone(), area);

    let inner_area = block.inner(area);
    if inner_area.height < 1 {
        return;
    }

    let mut tab_spans = vec![Span::styled(" Event Logs: ", *ALT_FG_STYLE)];
    for (i, log_name) in LOG_NAMES.iter().enumerate() {
        let is_selected = app_state.selected_log_index == i;
        let style = if is_selected { *SELECTION_STYLE } else { *DEFAULT_STYLE };
        tab_spans.extend([
            Span::styled(format!("[{}]", i + 1), *KEY_STYLE),
            Span::raw(":").style(style),
            Span::styled(log_name.to_string(), style),
            Span::raw("  ").style(*DEFAULT_STYLE),
        ]);
    }

    let tabs_paragraph = Paragraph::new(Line::from(tab_spans).alignment(Alignment::Left))
        .style(*DEFAULT_STYLE);
    let tabs_render_area = Rect { y: inner_area.y + inner_area.height.saturating_sub(1) / 2, height: 1, ..inner_area };
    frame.render_widget(tabs_paragraph, tabs_render_area);
}

fn render_event_table(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Events;
    let border_style = BORDER_STYLE.patch(Style::new().fg(if is_focused { WHITE } else { THEME_BORDER }));

    let block = Block::new()
        .title(Title::from(Span::styled(format!(" Events: {} ", app_state.selected_log_name), *TITLE_STYLE)).alignment(Alignment::Left).position(Position::Top))
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(BORDER_TYPE_THEME)
        .style(*DEFAULT_STYLE);

    if app_state.events.is_empty() {
        frame.render_widget(block.clone(), area);
        let inner_area = block.inner(area);
        let message = if app_state.active_filter.is_some() { "No events found matching filter criteria" } else { "No events found" };
        let centered_text = Paragraph::new(message)
            .style(DEFAULT_STYLE.patch(Style::new().fg(GRAY).add_modifier(Modifier::BOLD)))
            .alignment(Alignment::Center);
        let layout = Layout::vertical([Constraint::Percentage(40), Constraint::Length(3), Constraint::Percentage(40)]).split(inner_area);
        frame.render_widget(centered_text, layout[1]);
    } else {
        let event_rows: Vec<Row> = app_state.events.iter().map(|event| {
            let level_style = match event.level.as_str() {
                "Warning" => *WARN_FG_STYLE,
                "Error" | "Critical" => *ERROR_FG_STYLE,
                _ => *DEFAULT_STYLE,
            };
            Row::new([
                Cell::from(event.level.clone()).style(level_style),
                Cell::from(event.datetime.clone()),
                Cell::from(event.source.clone()),
                Cell::from(event.id.clone()),
            ]).style(*DEFAULT_STYLE)
        }).collect();

        let sort_indicator = if app_state.sort_descending { " ↓" } else { " ↑" };
        let header = Row::new([
            Cell::from("Level").style(*HEADER_STYLE),
            Cell::from(format!("Date and Time{}", sort_indicator)).style(*HEADER_STYLE),
            Cell::from("Source").style(*HEADER_STYLE),
            Cell::from("Event ID").style(*HEADER_STYLE),
        ]).style(*HEADER_ROW_STYLE).height(1);

        let table = Table::new(event_rows, [
            Constraint::Length(11),
            Constraint::Length(22),
            Constraint::Percentage(60),
            Constraint::Length(10),
        ])
        .header(header)
        .block(block)
        .highlight_style(*SELECTION_STYLE)
        .highlight_symbol(" ")
        .column_spacing(1)
        .style(*DEFAULT_STYLE);

        frame.render_stateful_widget(table, area, &mut app_state.table_state);
    }
}

fn render_preview_panel(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Preview;
    let border_style = BORDER_STYLE.patch(Style::new().fg(if is_focused { WHITE } else { THEME_BORDER }));

    let mut title_text: String;
    let mut content_to_display: String;
    let mut wrap_behavior: Wrap;
    let mut total_lines: usize = 0;

    match app_state.preview_view_mode {
        PreviewViewMode::Formatted => {
            title_text = " Event Details (Formatted) ".to_string();
            content_to_display = app_state.preview_formatted_content.clone()
                .unwrap_or_else(|| "<No event selected or error loading details>".to_string());
            wrap_behavior = Wrap { trim: true };
        }
        PreviewViewMode::RawXml => {
            title_text = " Event Details (Raw XML) ".to_string();
            match &app_state.preview_raw_xml {
                Some(raw_xml) => {
                    // Attempt to pretty-print the XML
                    match helpers::pretty_print_xml(raw_xml) {
                        Ok(pretty_xml) => {
                            content_to_display = pretty_xml;
                            title_text = " Event Details (Pretty XML) ".to_string(); // Update title
                        }
                        Err(e) => {
                            // If pretty-printing fails, show raw XML with an error message
                            content_to_display = format!(
                                "<Failed to pretty-print XML: {}. Displaying raw XML.>\n\n{}",
                                e,
                                raw_xml
                            );
                             title_text = " Event Details (Raw XML - Error) ".to_string();
                        }
                    }
                }
                None => {
                    content_to_display = "<No event selected or raw XML unavailable>".to_string();
                }
            }
            // XML should generally not be wrapped, allow horizontal scrolling
            wrap_behavior = Wrap { trim: false };
        }
    }
    // Calculate total lines *after* potentially pretty-printing
    total_lines = content_to_display.lines().count();

    let block = Block::new()
        .title(Title::from(Span::styled(title_text, *TITLE_STYLE)).alignment(Alignment::Left).position(Position::Top))
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(BORDER_TYPE_THEME)
        .style(*DEFAULT_STYLE);

    let inner_content_area = block.inner(area);
    let available_height = inner_content_area.height;

    // Clamp scroll based on the final content's line count
    if total_lines > 0 {
        let max_scroll = total_lines.saturating_sub(available_height as usize).max(0);
        app_state.preview_scroll = app_state.preview_scroll.min(max_scroll);
    } else {
         app_state.preview_scroll = 0;
    }

    // Use horizontal scroll offset = 0 for now. Add if needed later.
    let scroll_offset = (app_state.preview_scroll as u16, 0);

    let preview_paragraph = Paragraph::new(content_to_display)
        .wrap(wrap_behavior)
        .scroll(scroll_offset)
        .style(*DEFAULT_STYLE);

    frame.render_widget(block, area);
    frame.render_widget(preview_paragraph, inner_content_area);

    // Render scroll indicator based on final line count
     if total_lines > available_height as usize {
         render_scroll_indicator(frame, inner_content_area, app_state.preview_scroll + 1, total_lines, *TITLE_STYLE);
     }
}

// --- Dialog Rendering ---

fn render_status_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(status_dialog) = &app_state.status_dialog {
        if status_dialog.visible {
            let frame_width = frame.size().width;
            let frame_height = frame.size().height;

            // --- Dynamic Size Calculation ---
            let title_width = status_dialog.title.len() as u16;
            // Estimate message width needs (consider longest line if multi-line)
            let message_lines: Vec<&str> = status_dialog.message.lines().collect();
            let max_message_line_width = message_lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

            let min_width = 20; // Minimum dialog width
            let max_width_pct = 0.8; // Max width as percentage of frame width
            let h_padding = 2; // Horizontal padding + border

            // Calculate desired width
            let desired_width = (title_width.max(max_message_line_width) + h_padding)
                .max(min_width)
                .min((frame_width as f32 * max_width_pct) as u16);

            // Estimate wrapped lines needed for the message
            let effective_content_width = desired_width.saturating_sub(2); // Subtract borders
            let mut estimated_lines = 0;
            if effective_content_width > 0 {
                 estimated_lines = message_lines.iter().map(|line| {
                    (line.len() as f32 / effective_content_width as f32).ceil() as u16
                 }).sum();
            }
             estimated_lines = estimated_lines.max(1); // Ensure at least one line for the message


            let min_height = 5; // Minimum dialog height (title + border + msg + border + bottom)
            let max_height_pct = 0.8; // Max height as percentage of frame height
            let v_padding = 4; // Top title (1), Bottom title (1), Borders (2)

            // Calculate desired height
            let desired_height = (estimated_lines + v_padding)
                .max(min_height)
                .min((frame_height as f32 * max_height_pct) as u16);

            // --- Centered Rect ---
            let dialog_area = helpers::centered_fixed_rect(desired_width, desired_height, frame.size());

            // --- Render ---
            frame.render_widget(Clear, dialog_area); // Clear the area

            let dialog_style = if status_dialog.is_error {
                *DIALOG_ERROR_STYLE
            } else {
                *DIALOG_DEFAULT_STYLE
            };
            let dialog_block = create_dialog_block(
                &status_dialog.title,
                STATUS_DISMISS_TITLE.clone(),
                dialog_style,
            );

            frame.render_widget(dialog_block.clone(), dialog_area);
            let content_area = dialog_block.inner(dialog_area);

            // Create paragraph with wrapping and centering
            let message_paragraph = Paragraph::new(status_dialog.message.clone())
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center) // Center text horizontally
                .style(dialog_style);

            // Render paragraph in the content area
            // Note: Ratatui Paragraph doesn't vertically center block content easily after wrapping.
            // Centering the dynamic dialog vertically provides the main vertical centering effect.
            frame.render_widget(message_paragraph, content_area);
        }
    }
}

fn render_search_bar(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_searching {
        let search_width = 40.min(frame.size().width.saturating_sub(4));
        let search_height = 3;
        let y_pos = frame.size().height.saturating_sub(search_height + 2);
        let x_pos = (frame.size().width.saturating_sub(search_width)) / 2;
        let search_area = Rect::new(x_pos, y_pos, search_width, search_height);

        let search_block = Block::new()
            .title(SEARCH_BAR_TITLE.clone())
            .borders(Borders::ALL)
            .border_style(*DIALOG_DEFAULT_STYLE)
            .border_type(BORDER_TYPE_THEME)
            .style(*DIALOG_DEFAULT_STYLE);

        let search_text = format!(" {}_", app_state.search_term);
        let search_paragraph = Paragraph::new(search_text)
            .block(search_block)
            .style(*DIALOG_SELECTION_STYLE);

        frame.render_widget(Clear, search_area);
        frame.render_widget(search_paragraph, search_area);
    }
}

fn render_filter_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_filter_dialog_visible {
        const FILTER_LIST_MAX_HEIGHT: u16 = 5;
        let dialog_width = 60;
        let is_source_focused = app_state.filter_dialog_focus == FilterFieldFocus::Source;
        let list_visible = is_source_focused && !app_state.filter_dialog_filtered_sources.is_empty();
        let list_render_height = if list_visible {
            FILTER_LIST_MAX_HEIGHT.min(app_state.filter_dialog_filtered_sources.len() as u16).max(1)
        } else {
            0
        };

        const SOURCE_LABEL_HEIGHT: u16 = 1;
        const SOURCE_INPUT_HEIGHT: u16 = 1;
        const EVENT_ID_LABEL_HEIGHT: u16 = 1;
        const EVENT_ID_INPUT_HEIGHT: u16 = 1;
        const LEVEL_SELECT_HEIGHT: u16 = 1;
        const BUTTON_SPACER_HEIGHT: u16 = 1;
        const BUTTON_ROW_HEIGHT: u16 = 1;
        const BORDERS_HEIGHT: u16 = 2;
        const INNER_MARGIN_HEIGHT: u16 = 2;

        let total_inner_content_height = SOURCE_LABEL_HEIGHT
            + SOURCE_INPUT_HEIGHT
            + FILTER_LIST_MAX_HEIGHT
            + EVENT_ID_LABEL_HEIGHT
            + EVENT_ID_INPUT_HEIGHT
            + LEVEL_SELECT_HEIGHT
            + BUTTON_SPACER_HEIGHT
            + BUTTON_ROW_HEIGHT;

        let dialog_height = total_inner_content_height + INNER_MARGIN_HEIGHT + BORDERS_HEIGHT;
        let dialog_area = helpers::centered_fixed_rect(dialog_width, dialog_height.min(frame.size().height), frame.size());

        frame.render_widget(Clear, dialog_area);
        let dialog_block = create_dialog_block(
            "Filter Events",
            FILTER_CANCEL_TITLE.clone(),
            *DIALOG_DEFAULT_STYLE,
        );
        let inner_area = dialog_block.inner(dialog_area);
        frame.render_widget(dialog_block, dialog_area);

        let constraints = vec![
            Constraint::Length(SOURCE_LABEL_HEIGHT),
            Constraint::Length(SOURCE_INPUT_HEIGHT),
            Constraint::Length(list_render_height),
            Constraint::Length(EVENT_ID_LABEL_HEIGHT),
            Constraint::Length(EVENT_ID_INPUT_HEIGHT),
            Constraint::Length(LEVEL_SELECT_HEIGHT),
            Constraint::Length(BUTTON_SPACER_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(BUTTON_ROW_HEIGHT),
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(constraints)
            .split(inner_area);

        let mut chunk_index = 0;
        let base_text_style = *DIALOG_DEFAULT_STYLE;

        if chunk_index < chunks.len() { frame.render_widget(Paragraph::new("Source:").style(base_text_style), chunks[chunk_index]); chunk_index += 1; }
        let source_style = if is_source_focused { *DIALOG_SELECTION_STYLE } else { base_text_style };
        let source_input_display = if is_source_focused {
            format!("{}_", app_state.filter_dialog_source_input)
        } else if app_state.filter_dialog_source_input.is_empty() {
            "[Any Source]".to_string()
        } else {
            format!(" {}", app_state.filter_dialog_source_input)
        };
        if chunk_index < chunks.len() { frame.render_widget(Paragraph::new(source_input_display).style(source_style), chunks[chunk_index]); chunk_index += 1; }

        if chunk_index < chunks.len() {
            if list_visible {
                let list_items: Vec<ListItem> = app_state.filter_dialog_filtered_sources.iter()
                    .map(|(_, name)| ListItem::new(name.clone()).style(base_text_style))
                    .collect();
                let list = List::new(list_items)
                    .highlight_style(*SELECTION_STYLE)
                    .style(base_text_style)
                    .highlight_symbol(">");
                let mut list_state = ListState::default();
                list_state.select(app_state.filter_dialog_filtered_source_selection);
                frame.render_stateful_widget(list, chunks[chunk_index], &mut list_state);
            }
            chunk_index += 1;
        }

        if chunk_index < chunks.len() { frame.render_widget(Paragraph::new("Event ID:").style(base_text_style), chunks[chunk_index]); chunk_index += 1; }
        let is_eventid_focused = app_state.filter_dialog_focus == FilterFieldFocus::EventId;
        let event_id_input_style = if is_eventid_focused { *DIALOG_SELECTION_STYLE } else { base_text_style };
        let event_id_text = if is_eventid_focused {
            format!("{}_", app_state.filter_dialog_event_id)
        } else {
            format!(" {}", app_state.filter_dialog_event_id)
        };
        if chunk_index < chunks.len() { frame.render_widget(Paragraph::new(event_id_text).style(event_id_input_style), chunks[chunk_index]); chunk_index += 1; }
        let level_text = Line::from(vec![
            Span::raw("Level: ").style(base_text_style),
            Span::styled("< ", *SELECTION_STYLE),
            Span::styled(app_state.filter_dialog_level.display_name(), *DIALOG_SELECTION_STYLE),
            Span::styled(" >", *SELECTION_STYLE),
        ]);
        if chunk_index < chunks.len() { frame.render_widget(Paragraph::new(level_text), chunks[chunk_index]); chunk_index += 1; }

        if chunk_index < chunks.len() { chunk_index += 1; }

        if chunk_index < chunks.len() {
            let apply_style = if app_state.filter_dialog_focus == FilterFieldFocus::Apply { *SELECTION_STYLE } else { base_text_style };
            let clear_style = if app_state.filter_dialog_focus == FilterFieldFocus::Clear { *SELECTION_STYLE } else { base_text_style };
            let button_line = Line::from(vec![
                Span::styled(" [ Apply ] ", apply_style),
                Span::raw(" ").style(base_text_style),
                Span::styled(" [ Clear ] ", clear_style),
            ]).alignment(Alignment::Center);
            frame.render_widget(Paragraph::new(button_line).style(base_text_style), chunks[chunk_index]);
        }
    }
}

fn render_help_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.help_dialog_visible {
        let help_width = 80.min(frame.size().width.saturating_sub(4));
        let help_height = 30.min(frame.size().height.saturating_sub(4));
        let help_area = helpers::centered_fixed_rect(help_width, help_height, frame.size());

        frame.render_widget(Clear, help_area);

        let help_dialog_title_text = format!(" Help - Event Commander (v{}) ", VERSION);
        let help_block = create_dialog_block(
            &help_dialog_title_text,
            HELP_DISMISS_TITLE.clone(),
            *DIALOG_DEFAULT_STYLE,
        );
        let content_area = help_block.inner(help_area);
        frame.render_widget(help_block, help_area);

        let help_text = HELP_TEXT_LINES.clone();
        let total_lines = help_text.len();
        let visible_height = content_area.height as usize;

        let max_scroll = total_lines.saturating_sub(visible_height);
        app_state.help_scroll_position = app_state.help_scroll_position.min(max_scroll);
        let current_scroll = app_state.help_scroll_position;

        let help_paragraph = Paragraph::new(help_text)
            .wrap(Wrap { trim: false })
            .style(*DEFAULT_STYLE)
            .scroll((current_scroll as u16, 0));

        frame.render_widget(help_paragraph, content_area);

        render_scroll_indicator(frame, content_area, current_scroll + 1, total_lines, *TITLE_STYLE);
    }
}

fn render_bottom_bar(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let mut spans = Vec::with_capacity(16);
    spans.extend([
        KEY_Q.clone(), Span::raw(" Quit | ").style(*FOOTER_STYLE),
        KEY_F1.clone(), Span::raw(" Help | ").style(*FOOTER_STYLE),
    ]);

    match app_state.focus {
        PanelFocus::Events => {
            spans.extend([
                Span::styled("[Enter]", *KEY_STYLE), Span::raw(" Details | ").style(*FOOTER_STYLE),
                KEY_S_SORT.clone(), Span::raw(" Sort | ").style(*FOOTER_STYLE),
                KEY_L_LEVEL.clone(), Span::raw(format!(" Lvl ({}) | ", app_state.get_current_level_name())).style(*FOOTER_STYLE),
                KEY_F_FILTER.clone(), Span::raw(format!(" Adv Filter ({}) | ", app_state.get_filter_status())).style(*FOOTER_STYLE),
                KEY_SLASH_SEARCH.clone(), Span::raw(" Search").style(*FOOTER_STYLE),
            ]);
            if app_state.last_search_term.is_some() {
                spans.extend([
                    Span::raw(" | ").style(*FOOTER_STYLE),
                    KEY_N_NEXT.clone(), Span::raw(" Next | ").style(*FOOTER_STYLE),
                    KEY_P_PREV.clone(), Span::raw(" Prev").style(*FOOTER_STYLE),
                ]);
            }
        }
        PanelFocus::Preview => {
            spans.extend([
                KEY_ESC_LEFT.clone(), Span::raw(" Return | ").style(*FOOTER_STYLE),
                KEY_V_TOGGLE.clone(), Span::raw(" Toggle View | ").style(*FOOTER_STYLE),
                KEY_S_SAVE.clone(), Span::raw(" Save | ").style(*FOOTER_STYLE),
                KEY_SCROLL.clone(), Span::raw(" Scroll").style(*FOOTER_STYLE),
            ]);
        }
    }

    if app_state.is_loading {
        spans.push(Span::raw(" | ").style(*FOOTER_STYLE));
        spans.push(Span::styled("Loading...", *ALT_FG_STYLE));
    }

    frame.render_widget(Paragraph::new(Line::from(spans).alignment(Alignment::Left)).style(*FOOTER_STYLE), area);
}