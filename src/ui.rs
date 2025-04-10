use lazy_static::lazy_static;
use ratatui::{
    prelude::*,
    text::{Line, Span, Text},
    widgets::block::{Position, Title},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
};

use crate::helpers;
use crate::models::{AppState, DetailsViewMode, FilterFieldFocus, PanelFocus, LOG_NAMES};

// --- Constants & Static Styles ---

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CYAN: Color = Color::Cyan;
const WHITE: Color = Color::White;
const GRAY: Color = Color::Gray;
const DARK_GRAY: Color = Color::DarkGray;
const BLUE: Color = Color::Blue;
const YELLOW: Color = Color::Yellow;
const RED: Color = Color::Red;
const GREEN: Color = Color::Green;
const MAGENTA: Color = Color::Magenta;

lazy_static! {
    // Common Styles
    static ref BOLD_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
    static ref BOLD_GRAY_STYLE: Style = Style::new().bold().fg(GRAY);
    static ref BOLD_YELLOW_STYLE: Style = Style::new().bold().fg(YELLOW);
    static ref BOLD_WHITE_STYLE: Style = Style::new().bold().fg(WHITE);
    static ref BOLD_BLUE_STYLE: Style = Style::new().bold().fg(BLUE);
    static ref BOLD_GREEN_STYLE: Style = Style::new().bold().fg(GREEN);
    static ref BOLD_MAGENTA_STYLE: Style = Style::new().bold().fg(MAGENTA);
    static ref DARK_GRAY_FG_STYLE: Style = Style::new().fg(DARK_GRAY);
    static ref HIGHLIGHT_BLUE_BG: Style = Style::new().bold().bg(BLUE);
    static ref HIGHLIGHT_DARK_GRAY_BG: Style = Style::new().bold().bg(DARK_GRAY);
    static ref HEADER_STYLE: Style = Style::new().fg(YELLOW).add_modifier(Modifier::BOLD);
    static ref HEADER_ROW_STYLE: Style = Style::new().bg(DARK_GRAY);
    static ref FOCUSED_STYLE: Style = Style::new().bg(DARK_GRAY);
    static ref UNFOCUSED_STYLE: Style = Style::new();
    static ref REVERSED_STYLE: Style = Style::new().add_modifier(Modifier::REVERSED);
    static ref UNDERLINED_STYLE: Style = Style::new().add_modifier(Modifier::UNDERLINED);
    static ref YELLOW_BORDER_STYLE: Style = Style::new().fg(YELLOW);
    static ref MAGENTA_BORDER_STYLE: Style = Style::new().fg(MAGENTA);
    static ref BLUE_BORDER_STYLE: Style = Style::new().fg(BLUE);

    // Keybinding Spans (Static Part)
    static ref KEY_Q: Span<'static> = Span::styled("[q]", *BOLD_GRAY_STYLE);
    static ref KEY_F1: Span<'static> = Span::styled("[F1]", *BOLD_GRAY_STYLE);
    static ref KEY_S_SORT: Span<'static> = Span::styled("[s]", *BOLD_GRAY_STYLE);
    static ref KEY_L_LEVEL: Span<'static> = Span::styled("[l]", *BOLD_GRAY_STYLE);
    static ref KEY_F_FILTER: Span<'static> = Span::styled("[f]", *BOLD_GRAY_STYLE);
    static ref KEY_SLASH_SEARCH: Span<'static> = Span::styled("[/]", *BOLD_GRAY_STYLE);
    static ref KEY_N_NEXT: Span<'static> = Span::styled("[n]", *BOLD_GRAY_STYLE);
    static ref KEY_P_PREV: Span<'static> = Span::styled("[p]", *BOLD_GRAY_STYLE);
    static ref KEY_ESC: Span<'static> = Span::styled("[Esc]", *BOLD_GRAY_STYLE);
    static ref KEY_V_TOGGLE: Span<'static> = Span::styled("[v]", *BOLD_GRAY_STYLE);
    static ref KEY_S_SAVE: Span<'static> = Span::styled("[s]", *BOLD_GRAY_STYLE);
    static ref KEY_ENTER_ESC: Span<'static> = Span::styled("[Enter/Esc]", *BOLD_WHITE_STYLE);

    // Static Titles / Lines
    static ref LOG_LIST_HELP_LINE: Line<'static> = Line::from(vec![
        KEY_Q.clone(), Span::raw(" quit "), KEY_F1.clone(), Span::raw(" help"),
    ]).alignment(Alignment::Center);
    static ref LOG_LIST_HELP_TITLE: Title<'static> = Title::from(LOG_LIST_HELP_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref EVENT_DETAILS_HELP_LINE: Line<'static> = Line::from(vec![
        KEY_ESC.clone(), Span::raw(" Dismiss "),
        KEY_V_TOGGLE.clone(), Span::raw(" Toggle View "),
        KEY_S_SAVE.clone(), Span::raw(" Save Event to Disk "),
    ]).alignment(Alignment::Center);
    static ref EVENT_DETAILS_HELP_TITLE: Title<'static> = Title::from(EVENT_DETAILS_HELP_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

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

    static ref SEARCH_BAR_TITLE: &'static str = "Find (Enter to search, Esc to cancel)";

    static ref HELP_DISMISS_TEXT_LINE: Line<'static> = Line::from(vec![
        Span::styled("[Esc]", BOLD_GRAY_STYLE.clone()),
        Span::raw(" Dismiss "),
        Span::styled(" ↑↓ PgUp/Dn Home/End ", BOLD_GRAY_STYLE.clone()),
        Span::raw(" Scroll "),
    ]).alignment(Alignment::Center);
    static ref HELP_DISMISS_TITLE: Title<'static> = Title::from(HELP_DISMISS_TEXT_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    // Help Dialog Text (Keep it static)
    static ref HELP_TEXT_LINES: Vec<Line<'static>> = vec![
        Line::from(Span::styled("Event Commander", BOLD_STYLE.clone().fg(CYAN))),
        Line::from("A simple TUI for browsing Windows Event Logs."),
        Line::from(""),
        Line::from(vec![
            Span::raw("Developed by: "),
            Span::styled("Toby Martin", Style::default().fg(GREEN)),
        ]),
        Line::from(vec![
            Span::raw("Source Code: "),
            Span::styled(
                "https://github.com/Dastari/event_commander",
                Style::default().fg(BLUE).add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("License: GPL-3.0-or-later", Style::default().fg(MAGENTA))),
        Line::from("  This program is free software: you can redistribute it and/or modify"),
        Line::from("  it under the terms of the GNU General Public License as published by"),
        Line::from("  the Free Software Foundation, either version 3 of the License, or"),
        Line::from("  (at your option) any later version. See LICENSE.txt for details."),
        Line::from(""),
        Line::from(Span::styled("--- Keybindings ---", *BOLD_YELLOW_STYLE)),
        Line::from(""),
        Line::from(Span::styled("Global:", Style::default().underlined())),
        Line::from(vec![Span::styled("  q        ", *BOLD_STYLE), Span::raw("Quit application")]),
        Line::from(vec![Span::styled("  F1       ", *BOLD_STYLE), Span::raw("Show this help screen")]),
        Line::from(vec![Span::styled("  Tab      ", *BOLD_STYLE), Span::raw("Cycle focus forward (Logs -> Events -> Preview)")]),
        Line::from(vec![Span::styled("  Shift+Tab", *BOLD_STYLE), Span::raw("Cycle focus backward")]),
        Line::from(""),
        Line::from(Span::styled("Log List Panel (Left):", Style::default().underlined())),
        Line::from(vec![Span::styled("  Up/Down  ", *BOLD_STYLE), Span::raw("Select previous/next log type")]),
        Line::from(vec![Span::styled("  Enter    ", *BOLD_STYLE), Span::raw("Focus Event List")]),
        Line::from(vec![Span::styled("  Right/Tab", *BOLD_STYLE), Span::raw("Focus Event List")]),
        Line::from(""),
        Line::from(Span::styled("Event List Panel (Middle):", Style::default().underlined())),
        Line::from(vec![Span::styled("  Up/Down  ", *BOLD_STYLE), Span::raw("Scroll event list up/down by one")]),
        Line::from(vec![Span::styled("  PgUp/PgDn", *BOLD_STYLE), Span::raw("Scroll event list up/down by page")]),
        Line::from(vec![Span::styled("  Home/g   ", *BOLD_STYLE), Span::raw("Go to top of event list")]),
        Line::from(vec![Span::styled("  End/G    ", *BOLD_STYLE), Span::raw("Go to bottom of event list (loads more if needed)")]),
        Line::from(vec![Span::styled("  Enter    ", *BOLD_STYLE), Span::raw("Show event details dialog")]),
        Line::from(vec![Span::styled("  Left/S-Tab", *BOLD_STYLE), Span::raw("Focus Log List")]),
        Line::from(vec![Span::styled("  Tab      ", *BOLD_STYLE), Span::raw("Focus Preview Panel")]),
        Line::from(vec![Span::styled("  s        ", *BOLD_STYLE), Span::raw("Toggle sort direction (Date/Time)")]),
        Line::from(vec![Span::styled("  l        ", *BOLD_STYLE), Span::raw("Cycle level filter (All -> Info -> Warn -> Error -> All)")]),
        Line::from(vec![Span::styled("  f        ", *BOLD_STYLE), Span::raw("Open filter dialog")]),
        Line::from(vec![Span::styled("  /        ", *BOLD_STYLE), Span::raw("Start search input")]),
        Line::from(vec![Span::styled("  n        ", *BOLD_STYLE), Span::raw("Find next search match")]),
        Line::from(vec![Span::styled("  p/N      ", *BOLD_STYLE), Span::raw("Find previous search match")]),
        Line::from(""),
        Line::from(Span::styled("Preview Panel (Bottom Right):", Style::default().underlined())),
        Line::from(vec![Span::styled("  Up/Down  ", *BOLD_STYLE), Span::raw("Scroll preview up/down by one line")]),
        Line::from(vec![Span::styled("  PgUp/PgDn", *BOLD_STYLE), Span::raw("Scroll preview up/down by page")]),
        Line::from(vec![Span::styled("  Home/g   ", *BOLD_STYLE), Span::raw("Go to top of preview")]),
        Line::from(vec![Span::styled("  Left/S-Tab", *BOLD_STYLE), Span::raw("Focus Event List")]),
        Line::from(vec![Span::styled("  Tab      ", *BOLD_STYLE), Span::raw("Focus Log List")]),
        Line::from(""),
        Line::from(Span::styled("Event Details Dialog:", Style::default().underlined())),
        Line::from(vec![Span::styled("  Esc      ", *BOLD_STYLE), Span::raw("Dismiss dialog")]),
        Line::from(vec![Span::styled("  v        ", *BOLD_STYLE), Span::raw("Toggle view (Formatted / Raw XML)")]),
        Line::from(vec![Span::styled("  s        ", *BOLD_STYLE), Span::raw("Save current event XML to disk")]),
        Line::from(vec![Span::styled("  Up/Down  ", *BOLD_STYLE), Span::raw("Scroll content up/down by one line")]),
        Line::from(vec![Span::styled("  PgUp/PgDn", *BOLD_STYLE), Span::raw("Scroll content up/down by page")]),
        Line::from(vec![Span::styled("  Home/g   ", *BOLD_STYLE), Span::raw("Go to top of content")]),
        Line::from(vec![Span::styled("  End/G    ", *BOLD_STYLE), Span::raw("Go to bottom of content")]),
        Line::from(""),
        Line::from(Span::styled("Filter Dialog:", Style::default().underlined())),
        Line::from(vec![Span::styled("  Esc      ", *BOLD_STYLE), Span::raw("Cancel filtering")]),
        Line::from(vec![Span::styled("  Tab      ", *BOLD_STYLE), Span::raw("Cycle focus forward (Source -> ID -> Level -> Apply -> Clear)")]),
        Line::from(vec![Span::styled("  Shift+Tab", *BOLD_STYLE), Span::raw("Cycle focus backward")]),
        Line::from(vec![Span::styled("  Enter    ", *BOLD_STYLE), Span::raw("Confirm selection / Move to next field / Apply/Clear")]),
        Line::from(vec![Span::styled("  Chars    ", *BOLD_STYLE), Span::raw("Type in Source/Event ID fields")]),
        Line::from(vec![Span::styled("  Backspace", *BOLD_STYLE), Span::raw("Delete character in Source/Event ID fields")]),
        Line::from(vec![Span::styled("  Up/Down  ", *BOLD_STYLE), Span::raw("Select source from filtered list (when Source focused)")]),
        Line::from(vec![Span::styled("  Left/Right", *BOLD_STYLE), Span::raw("Cycle level filter (when Level focused)")]),
        Line::from(""),
        Line::from(Span::styled("Search Input:", Style::default().underlined())),
        Line::from(vec![Span::styled("  Esc      ", *BOLD_STYLE), Span::raw("Cancel search")]),
        Line::from(vec![Span::styled("  Enter    ", *BOLD_STYLE), Span::raw("Perform search")]),
        Line::from(vec![Span::styled("  Chars    ", *BOLD_STYLE), Span::raw("Type search term")]),
        Line::from(vec![Span::styled("  Backspace", *BOLD_STYLE), Span::raw("Delete character")]),
        Line::from(""),
        Line::from(Span::styled("Status/Confirmation Dialog:", Style::default().underlined())),
        Line::from(vec![Span::styled("  Enter/Esc", *BOLD_STYLE), Span::raw("Dismiss dialog")]),
    ];
}

// --- Helper Functions ---

/// Creates a standard block for dialogs.
fn create_dialog_block(title: &str, bottom_title: Title<'static>, border_style: Style) -> Block<'static> {
    Block::default()
        .title(title.to_string()) // Dialog title is often dynamic
        .title(bottom_title)
        .borders(Borders::ALL)
        .border_style(border_style)
}

/// Renders a scroll indicator `[current/total]` at the top-right of the area.
fn render_scroll_indicator(
    frame: &mut Frame,
    area: Rect,
    current_line: usize, // 1-based for display
    total_lines: usize,
    color: Color,
) {
    if total_lines > area.height as usize && area.width > 5 { // Ensure enough space
        let scroll_info = format!("[{}/{}]", current_line, total_lines);
        let scroll_width = scroll_info.len() as u16;
        let scroll_rect = Rect::new(
            area.right().saturating_sub(scroll_width + 1), // +1 for border/padding
            area.y,
            scroll_width,
            1,
        );
        let scroll_indicator = Paragraph::new(scroll_info).style(Style::default().fg(color));
        frame.render_widget(scroll_indicator, scroll_rect);
    }
}

/// Determines the border style based on focus.
fn get_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(CYAN)
    } else {
        Style::default().fg(WHITE)
    }
}

/// Determines the highlight background style based on focus.
fn get_highlight_bg(focused: bool) -> Style {
    if focused {
        *HIGHLIGHT_BLUE_BG
    } else {
        *HIGHLIGHT_DARK_GRAY_BG
    }
}

// --- Main UI Rendering ---

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

    // Render dialogs if they're visible (order matters for layering)
    render_status_dialog(frame, app_state); // Render status/confirm first
    render_event_details_dialog(frame, app_state);
    render_filter_dialog(frame, app_state);
    render_help_dialog(frame, app_state);
    render_search_bar(frame, app_state); // Search bar on top
}

// --- Panel Rendering ---

fn render_log_list(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Logs;
    let log_items: Vec<ListItem> = LOG_NAMES.iter().map(|&name| ListItem::new(name)).collect();

    let log_list_block = Block::default()
        .title("Event Viewer (Local)")
        .title(LOG_LIST_HELP_TITLE.clone())
        .borders(Borders::ALL)
        .border_style(get_border_style(is_focused));

    let log_list = List::new(log_items)
        .block(log_list_block)
        .highlight_style(get_highlight_bg(is_focused))
        .highlight_symbol("> ");

    let mut log_list_state = ListState::default();
    log_list_state.select(Some(app_state.selected_log_index));
    frame.render_stateful_widget(log_list, area, &mut log_list_state);
}

fn render_event_table(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Events;

    // Dynamic parts of help line
    let next_prev_style = if app_state.last_search_term.is_some() {
        *BOLD_GRAY_STYLE
    } else {
        *DARK_GRAY_FG_STYLE
    };
    let event_table_help_line = Line::from(vec![
        KEY_S_SORT.clone(), Span::raw(" sort "),
        KEY_L_LEVEL.clone(), Span::raw(format!(" level ({}) ", app_state.get_current_level_name())),
        KEY_F_FILTER.clone(), Span::raw(format!(" filter ({}) ", app_state.get_filter_status())),
        KEY_SLASH_SEARCH.clone(), Span::raw(" search "),
        Span::styled("[n]", next_prev_style), Span::raw(" next "), // Use local style here
        Span::styled("[p]", next_prev_style), Span::raw(" prev"), // Use local style here
    ]).alignment(Alignment::Center);
    let event_table_help_title = Title::from(event_table_help_line)
        .position(Position::Bottom).alignment(Alignment::Center);

    let event_table_block = Block::default()
        .title(format!("Events: {}", app_state.selected_log_name)) // Dynamic title
        .title(event_table_help_title)
        .borders(Borders::ALL)
        .border_style(get_border_style(is_focused));

    // Check if events list is empty
    if app_state.events.is_empty() {
        let message = if app_state.active_filter.is_some() {
            "No events found matching filter criteria"
        } else {
            "No events found"
        };

        // Create a layout for vertical centering
        let inner_area = event_table_block.inner(area);
        let vertical_layout = Layout::vertical([
            Constraint::Percentage(40), // Top space
            Constraint::Length(3),      // Message height
            Constraint::Percentage(40), // Bottom space
        ]).split(inner_area);

        frame.render_widget(event_table_block, area);

        let centered_text = Paragraph::new(message)
            .style(Style::default().fg(GRAY).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);

        frame.render_widget(centered_text, vertical_layout[1]);
    } else {
        // Prepare table rows (dynamic data)
        let event_rows: Vec<Row> = app_state.events.iter().map(|event| {
            let level_style = match event.level.as_str() {
                "Warning" => Style::default().fg(YELLOW),
                "Error" | "Critical" => Style::default().fg(RED),
                _ => Style::default(),
            };
            Row::new([
                Cell::from(event.level.clone()).style(level_style),
                Cell::from(event.datetime.clone()),
                Cell::from(event.source.clone()),
                Cell::from(event.id.clone()),
            ])
        }).collect();

        // Prepare header (partially dynamic)
        let sort_indicator = if app_state.sort_descending { " ↓" } else { " ↑" };
        let datetime_header = format!("Date and Time{}", sort_indicator);
        let header_cells = [
            Cell::from("Level").style(*HEADER_STYLE),
            Cell::from(datetime_header).style(*HEADER_STYLE), // Dynamic text, static style
            Cell::from("Source").style(*HEADER_STYLE),
            Cell::from("Event ID").style(*HEADER_STYLE),
        ];
        let header = Row::new(header_cells).style(*HEADER_ROW_STYLE).height(1);

        let widths = [
            Constraint::Length(11),
            Constraint::Length(22),
            Constraint::Percentage(60),
            Constraint::Length(10),
        ];

        let event_table = Table::new(event_rows, widths)
            .header(header)
            .block(event_table_block)
            .highlight_style(*REVERSED_STYLE)
            .highlight_symbol(">> ")
            .column_spacing(1);

        frame.render_stateful_widget(event_table, area, &mut app_state.table_state);
    }
}

fn render_preview_panel(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Preview;

    let preview_block = Block::default()
        .title("Event Message Preview")
        .borders(Borders::ALL)
        .border_style(get_border_style(is_focused));

    // Determine preview message (dynamic)
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

    // Adjust scroll based on content
    let message_lines = preview_message.lines().count() as u16;
    let available_height = area.height.saturating_sub(2); // Account for borders
    app_state.preview_scroll = app_state
        .preview_scroll
        .min(message_lines.saturating_sub(available_height.max(1)));

    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        .scroll((app_state.preview_scroll, 0));

    frame.render_widget(preview_paragraph, area);

    // Render version string (static but positioned dynamically)
    let version_string = format!("v{}", VERSION);
    let version_width = version_string.len() as u16;
    if area.width > version_width + 2 && area.height > 1 {
        let version_x = area.right() - version_width - 1;
        let version_y = area.bottom() - 1;
        let version_rect = Rect::new(version_x, version_y, version_width, 1);
        let version_paragraph = Paragraph::new(version_string).style(*DARK_GRAY_FG_STYLE);
        frame.render_widget(version_paragraph, version_rect);
    }
}

// --- Dialog Rendering ---

fn render_event_details_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            // Use helper for centered rect
            let dialog_width = 70.min(frame.size().width.saturating_sub(4));
            let dialog_height = 20.min(frame.size().height.saturating_sub(4));
            let dialog_area = helpers::centered_fixed_rect(dialog_width, dialog_height, frame.size());

            frame.render_widget(Clear, dialog_area);

            // Dynamic title part
            let view_mode_suffix = match event_details.view_mode {
                DetailsViewMode::Formatted => " (Formatted)",
                DetailsViewMode::RawXml => " (Raw XML)",
            };
            let dialog_title = format!("{}{}", event_details.title, view_mode_suffix);

            // Use helper for block creation
            let dialog_block = create_dialog_block(
                &dialog_title,
                EVENT_DETAILS_HELP_TITLE.clone(),
                *BLUE_BORDER_STYLE,
            );
            frame.render_widget(dialog_block.clone(), dialog_area); // Clone block for inner area calc

            let content_area = dialog_block.inner(dialog_area);

            // Content and scrolling logic (remains largely the same)
            event_details.current_visible_height = (content_area.height as usize).max(1);
            let visible_height = event_details.current_visible_height;
            let content = event_details.current_content();
            let content_lines: Vec<Line> = content.lines().map(Line::from).collect(); // Convert to Lines for Paragraph
            let total_lines = content_lines.len();

            let start_line = event_details.scroll_position.min(total_lines.saturating_sub(1));
            // We don't need end_line calculation for Paragraph scroll

            let wrap_behavior = match event_details.view_mode {
                DetailsViewMode::Formatted => Wrap { trim: true },
                DetailsViewMode::RawXml => Wrap { trim: false },
            };

            let content_paragraph = Paragraph::new(Text::from(content_lines))
                .wrap(wrap_behavior)
                .style(Style::default().fg(WHITE))
                .scroll((start_line as u16, 0)); // Use start_line for scroll offset

            // frame.render_widget(Clear, content_area); // Clear happens before block render
            frame.render_widget(content_paragraph, content_area);

            // Use helper for scroll indicator
            render_scroll_indicator(
                frame,
                content_area,
                start_line + 1, // Display 1-based index
                total_lines,
                BLUE,
            );
        }
    }
}

fn render_status_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(status_dialog) = &app_state.status_dialog {
        if status_dialog.visible {
            let dialog_width = 60.min(frame.size().width.saturating_sub(4));
            let dialog_height = 10.min(frame.size().height.saturating_sub(4));
            let dialog_area = helpers::centered_fixed_rect(dialog_width, dialog_height, frame.size());

            frame.render_widget(Clear, dialog_area);

            let border_color = if status_dialog.is_error { RED } else { GREEN };
            let border_style = Style::default().fg(border_color);

            // Use helper for block creation
            let dialog_block = create_dialog_block(
                &status_dialog.title, // Title is dynamic
                STATUS_DISMISS_TITLE.clone(),
                border_style,
            );
            frame.render_widget(dialog_block.clone(), dialog_area); // Clone for inner calc

            let content_area = dialog_block.inner(dialog_area);

            // Render dynamic message
            let message_paragraph = Paragraph::new(status_dialog.message.clone())
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(WHITE));
            frame.render_widget(message_paragraph, content_area);
        }
    }
}

fn render_search_bar(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_searching {
        let search_width = 40.min(frame.size().width.saturating_sub(4));
        let search_height = 3;
        // Position near bottom, but use centered_fixed_rect logic manually for vertical position
        let y_pos = frame.size().height.saturating_sub(search_height + 2);
        let x_pos = (frame.size().width.saturating_sub(search_width)) / 2;
        let search_area = Rect::new(x_pos, y_pos, search_width, search_height);


        // Use static title string and border style
        let search_block = Block::default()
            .title(*SEARCH_BAR_TITLE)
            .borders(Borders::ALL)
            .border_style(*YELLOW_BORDER_STYLE);

        // Dynamic search text with cursor simulation
        let search_text = format!("{}_", app_state.search_term);
        let search_paragraph = Paragraph::new(search_text)
            .block(search_block.clone()) // Clone block
            .style(Style::default().fg(WHITE));

        frame.render_widget(Clear, search_area); // Clear before rendering
        frame.render_widget(search_paragraph, search_area);
    }
}

fn render_filter_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.is_filter_dialog_visible {
        let dialog_width = 60;
        let is_source_focused = app_state.filter_dialog_focus == FilterFieldFocus::Source;
        let list_visible = is_source_focused && !app_state.filter_dialog_filtered_sources.is_empty();
        let list_height = if list_visible {
            5.min(app_state.filter_dialog_filtered_sources.len() as u16).max(1)
        } else {
            0 // No height if not visible or empty
        };

        // Calculate height based on components
        let source_label_height = 1;
        let source_input_height = 1;
        let source_list_height = list_height;
        let source_area_height = source_label_height + source_input_height + source_list_height;
        let event_id_label_height = 1;
        let event_id_input_height = 1;
        let level_select_height = 1;
        let button_spacer_height = 1;
        let button_row_height = 1;

        let required_inner_height = source_area_height + event_id_label_height + event_id_input_height + level_select_height + button_spacer_height + button_row_height;
        let dialog_height = required_inner_height + 2 + 1; // +2 borders, +1 margin

        let dialog_area = helpers::centered_fixed_rect(
            dialog_width,
            dialog_height.min(frame.size().height), // Clamp height
            frame.size(),
        );
        frame.render_widget(Clear, dialog_area);

        // Use helper for block
        let dialog_block = create_dialog_block(
            "Filter Events",
            FILTER_CANCEL_TITLE.clone(),
            *MAGENTA_BORDER_STYLE,
        );
        let inner_area = dialog_block.inner(dialog_area);
        frame.render_widget(dialog_block.clone(), dialog_area); // Clone for inner calc


        // Layout inside the dialog
        let constraints = vec![
            Constraint::Length(source_label_height),
            Constraint::Length(source_input_height),
            Constraint::Length(source_list_height), // Dynamic list height
            Constraint::Length(event_id_label_height),
            Constraint::Length(event_id_input_height),
            Constraint::Length(level_select_height),
            Constraint::Min(button_spacer_height), // Spacer
            Constraint::Length(button_row_height),
        ];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1) // Margin inside the block
            .constraints(constraints.iter().filter(|&&c| match c { Constraint::Length(h) => h > 0, _ => true })) // Filter out zero-height constraints
            .split(inner_area);

        let mut chunk_index = 0;

        // Source Field
        let is_source_focused = app_state.filter_dialog_focus == FilterFieldFocus::Source;
        frame.render_widget(Paragraph::new("Source:"), chunks[chunk_index]); chunk_index += 1;
        let source_style = if is_source_focused { *FOCUSED_STYLE } else { *UNFOCUSED_STYLE };
        let source_input_display = if is_source_focused {
            format!("{}_", app_state.filter_dialog_source_input)
        } else if app_state.filter_dialog_source_input.is_empty() {
            "[Any Source]".to_string()
        } else {
            app_state.filter_dialog_source_input.clone()
        };
        frame.render_widget(Paragraph::new(source_input_display).style(source_style), chunks[chunk_index]); chunk_index += 1;

        // Source List (if visible)
        if list_visible {
            let list_items: Vec<ListItem> = app_state.filter_dialog_filtered_sources.iter()
                .map(|(_, name)| ListItem::new(name.clone()))
                .collect();
            let list = List::new(list_items)
                .highlight_style(*HIGHLIGHT_BLUE_BG)
                .highlight_symbol("> ");
            let mut list_state = ListState::default();
            list_state.select(app_state.filter_dialog_filtered_source_selection);
            frame.render_stateful_widget(list, chunks[chunk_index], &mut list_state);
            chunk_index += 1;
        }

        // Event ID Field
        let is_eventid_focused = app_state.filter_dialog_focus == FilterFieldFocus::EventId;
        frame.render_widget(Paragraph::new("Event ID:"), chunks[chunk_index]); chunk_index += 1;
        let event_id_input_style = if is_eventid_focused { *FOCUSED_STYLE } else { *UNFOCUSED_STYLE };
        let event_id_text = if is_eventid_focused {
            format!("{}_", app_state.filter_dialog_event_id)
        } else {
            app_state.filter_dialog_event_id.clone()
        };
        frame.render_widget(Paragraph::new(event_id_text).style(event_id_input_style), chunks[chunk_index]); chunk_index += 1;

        // Level Selector
        let is_level_focused = app_state.filter_dialog_focus == FilterFieldFocus::Level;
        let level_style = if is_level_focused {
             FOCUSED_STYLE.clone().add_modifier(Modifier::BOLD)
        } else {
            *UNFOCUSED_STYLE
        };
        let level_text = Line::from(vec![
            Span::raw("Level: "),
            Span::styled("< ", Style::default().fg(YELLOW)),
            Span::styled(app_state.filter_dialog_level.display_name(), level_style),
            Span::styled(" >", Style::default().fg(YELLOW)),
        ]);
        frame.render_widget(Paragraph::new(level_text), chunks[chunk_index]); chunk_index += 1;


        // Spacer (rendered implicitly by Min constraint)
        //frame.render_widget(Paragraph::new(""), chunks[chunk_index]);
        chunk_index += 1;


        // Apply/Clear Buttons
        let apply_style = if app_state.filter_dialog_focus == FilterFieldFocus::Apply {
            FOCUSED_STYLE.clone().add_modifier(Modifier::BOLD)
        } else { *UNFOCUSED_STYLE };
        let clear_style = if app_state.filter_dialog_focus == FilterFieldFocus::Clear {
            FOCUSED_STYLE.clone().add_modifier(Modifier::BOLD)
        } else { *UNFOCUSED_STYLE };
        let apply_text = Span::styled(" [ Apply ] ", apply_style);
        let clear_text = Span::styled(" [ Clear ] ", clear_style);

        let button_layout = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[chunk_index]); // Use the last chunk
        frame.render_widget(Paragraph::new(apply_text).alignment(Alignment::Center), button_layout[0]);
        frame.render_widget(Paragraph::new(clear_text).alignment(Alignment::Center), button_layout[1]);
    }
}


fn render_help_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if app_state.help_dialog_visible {
        let help_width = 80.min(frame.size().width.saturating_sub(4));
        let help_height = 30.min(frame.size().height.saturating_sub(4));
        let help_area = helpers::centered_fixed_rect(help_width, help_height, frame.size());

        frame.render_widget(Clear, help_area);

        // Use static title and helper block
        let help_dialog_title = format!(" Help - Event Commander (v{}) ", VERSION);
        let help_block = create_dialog_block(
            &help_dialog_title,
            HELP_DISMISS_TITLE.clone(),
            *YELLOW_BORDER_STYLE,
        );
        let content_area = help_block.inner(help_area); // Calculate inner area *before* rendering block
        frame.render_widget(help_block, help_area);

        // Use static help text
        let help_text = HELP_TEXT_LINES.clone(); // Clone the static Vec<Line>
        let total_lines = help_text.len();
        let visible_height = content_area.height as usize;

        // Scroll calculation
        let max_scroll = total_lines.saturating_sub(visible_height);
        app_state.help_scroll_position = app_state.help_scroll_position.min(max_scroll);
        let current_scroll = app_state.help_scroll_position;

        let help_paragraph = Paragraph::new(help_text) // Pass the cloned Vec<Line>
            .wrap(Wrap { trim: false }) // Help text needs explicit line breaks
            .style(Style::default().fg(WHITE))
            .scroll((current_scroll as u16, 0));

        frame.render_widget(help_paragraph, content_area);

        // Use helper for scroll indicator
        render_scroll_indicator(
            frame,
            content_area,
            current_scroll + 1, // Display 1-based index
            total_lines,
            YELLOW,
        );
    }
} 