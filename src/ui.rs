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

// --- Theme Constants ---
const THEME_BG: Color = Color::Blue;
const THEME_FG: Color = Color::LightCyan;
const THEME_BORDER: Color = Color::LightCyan;
const THEME_HIGHLIGHT_BG: Color = Color::Cyan;
const THEME_HIGHLIGHT_FG: Color = Color::Black;
const THEME_ALT_FG: Color = Color::LightYellow; // For headers, specific highlights
const THEME_ERROR_FG: Color = Color::Red;
const THEME_WARN_FG: Color = Color::Yellow;
const VERSION: &str = env!("CARGO_PKG_VERSION");

// Keep specific colors if needed, but prefer theme constants
const WHITE: Color = Color::White;
const GRAY: Color = Color::Gray;
const DARK_GRAY: Color = Color::DarkGray;
const RED: Color = Color::Red;
const GREEN: Color = Color::Green;
const MAGENTA: Color = Color::Magenta;

lazy_static! {
    // --- Core Theme Styles ---
    static ref DEFAULT_STYLE: Style = Style::new().bg(THEME_BG).fg(THEME_FG);
    static ref BORDER_STYLE: Style = Style::new().fg(THEME_BORDER);
    static ref SELECTION_STYLE: Style = Style::new().bg(THEME_HIGHLIGHT_BG).fg(THEME_HIGHLIGHT_FG);
    static ref ALT_FG_STYLE: Style = DEFAULT_STYLE.clone().fg(THEME_ALT_FG);
    static ref ERROR_FG_STYLE: Style = DEFAULT_STYLE.clone().fg(THEME_ERROR_FG);
    static ref WARN_FG_STYLE: Style = DEFAULT_STYLE.clone().fg(THEME_WARN_FG);
    static ref TITLE_STYLE: Style = SELECTION_STYLE.clone(); // Cyan bg, Black fg for titles in borders
    static ref DARK_GRAY_FG_STYLE: Style = DEFAULT_STYLE.clone().fg(DARK_GRAY); // For less important info like version

    // --- Component Styles Based on Theme ---
    static ref BOLD_STYLE: Style = DEFAULT_STYLE.clone().add_modifier(Modifier::BOLD);
    static ref HEADER_STYLE: Style = DEFAULT_STYLE.clone().fg(THEME_ALT_FG).add_modifier(Modifier::BOLD);
    static ref HEADER_ROW_STYLE: Style = DEFAULT_STYLE.clone(); // Use default background
    static ref INPUT_FOCUSED_STYLE: Style = SELECTION_STYLE.clone(); // Cyan bg, Black fg for focused inputs
    static ref INPUT_UNFOCUSED_STYLE: Style = DEFAULT_STYLE.clone();
    static ref REVERSED_STYLE: Style = DEFAULT_STYLE.clone().add_modifier(Modifier::REVERSED); // Keep for table highlight for now
    static ref UNDERLINED_STYLE: Style = DEFAULT_STYLE.clone().add_modifier(Modifier::UNDERLINED);

    // --- Keybinding Styles ---
    // Keys now use the main SELECTION_STYLE (Cyan bg, Black fg)
    static ref KEY_STYLE: Style = SELECTION_STYLE.clone();
    static ref KEY_Q: Span<'static> = Span::styled("[q]", KEY_STYLE.clone());
    static ref KEY_F1: Span<'static> = Span::styled("[F1]", KEY_STYLE.clone());
    static ref KEY_S_SORT: Span<'static> = Span::styled("[s]", KEY_STYLE.clone());
    static ref KEY_L_LEVEL: Span<'static> = Span::styled("[l]", KEY_STYLE.clone());
    static ref KEY_F_FILTER: Span<'static> = Span::styled("[f]", KEY_STYLE.clone());
    static ref KEY_SLASH_SEARCH: Span<'static> = Span::styled("[/]", KEY_STYLE.clone());
    static ref KEY_N_NEXT: Span<'static> = Span::styled("[n]", KEY_STYLE.clone());
    static ref KEY_P_PREV: Span<'static> = Span::styled("[p]", KEY_STYLE.clone());
    static ref KEY_ESC: Span<'static> = Span::styled("[Esc]", KEY_STYLE.clone());
    static ref KEY_V_TOGGLE: Span<'static> = Span::styled("[v]", KEY_STYLE.clone());
    static ref KEY_S_SAVE: Span<'static> = Span::styled("[s]", KEY_STYLE.clone());
    static ref KEY_ENTER_ESC: Span<'static> = Span::styled("[Enter/Esc]", KEY_STYLE.clone());

    // --- Static Titles / Lines --- (Update styles)
    // Log List Help Title not needed anymore

    static ref EVENT_DETAILS_HELP_LINE: Line<'static> = Line::from(vec![
        KEY_ESC.clone(), Span::raw(" Dismiss ").style(DEFAULT_STYLE.clone()),
        KEY_V_TOGGLE.clone(), Span::raw(" Toggle View ").style(DEFAULT_STYLE.clone()),
        KEY_S_SAVE.clone(), Span::raw(" Save Event to Disk ").style(DEFAULT_STYLE.clone()),
    ]).alignment(Alignment::Center);
    static ref EVENT_DETAILS_HELP_TITLE: Title<'static> = Title::from(EVENT_DETAILS_HELP_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref STATUS_DISMISS_LINE: Line<'static> = Line::from(vec![
        KEY_ENTER_ESC.clone(), Span::raw(" Dismiss ").style(DEFAULT_STYLE.clone()),
    ]).alignment(Alignment::Center);
    static ref STATUS_DISMISS_TITLE: Title<'static> = Title::from(STATUS_DISMISS_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref FILTER_CANCEL_LINE: Line<'static> = Line::from(vec![
        KEY_ESC.clone(), Span::raw(" Cancel").style(DEFAULT_STYLE.clone()),
    ]).alignment(Alignment::Center);
    static ref FILTER_CANCEL_TITLE: Title<'static> = Title::from(FILTER_CANCEL_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    static ref SEARCH_BAR_TITLE_STYLE: Style = TITLE_STYLE.clone();
    static ref SEARCH_BAR_TITLE: Title<'static> = Title::from(
        Span::styled(" Find (Enter to search, Esc to cancel) ", SEARCH_BAR_TITLE_STYLE.clone())
    ).alignment(Alignment::Left).position(Position::Top);

    static ref HELP_DISMISS_TEXT_LINE: Line<'static> = Line::from(vec![
        Span::styled("[Esc]", KEY_STYLE.clone()), // Use KEY_STYLE
        Span::raw(" Dismiss ").style(DEFAULT_STYLE.clone()),
        Span::styled(" [↑↓ PgUp/Dn Home/End] ", KEY_STYLE.clone()), // Style scroll keys
        Span::raw(" Scroll ").style(DEFAULT_STYLE.clone()),
    ]).alignment(Alignment::Center);
    static ref HELP_DISMISS_TITLE: Title<'static> = Title::from(HELP_DISMISS_TEXT_LINE.clone())
        .position(Position::Bottom).alignment(Alignment::Center);

    // Help Dialog Text (Update Styles)
    static ref HELP_TEXT_LINES: Vec<Line<'static>> = vec![
        Line::from(Span::styled("Event Commander", BOLD_STYLE.clone().fg(THEME_FG))),
        Line::from(Span::raw("A simple TUI for browsing Windows Event Logs.").style(DEFAULT_STYLE.clone())),
        Line::from(Span::raw("").style(DEFAULT_STYLE.clone())),
        Line::from(vec![
            Span::raw("Developed by: ").style(DEFAULT_STYLE.clone()),
            Span::styled("Toby Martin", DEFAULT_STYLE.clone().fg(GREEN)), // Keep specific color?
        ]),
        Line::from(vec![
            Span::raw("Source Code: ").style(DEFAULT_STYLE.clone()),
            Span::styled(
                "https://github.com/Dastari/event_commander",
                DEFAULT_STYLE.clone().fg(THEME_FG).add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(Span::raw("").style(DEFAULT_STYLE.clone())),
        Line::from(Span::styled("License: GPL-3.0-or-later", DEFAULT_STYLE.clone().fg(MAGENTA))), // Keep specific color?
        // ... rest of help lines need .style(*DEFAULT_STYLE) ...
        // This part is tedious, will shorten for brevity but apply style to all raw strings
        Line::from(Span::raw("  This program is free software...").style(DEFAULT_STYLE.clone())),
        Line::from(Span::raw("  it under the terms...").style(DEFAULT_STYLE.clone())),
        Line::from(Span::raw("  the Free Software Foundation...").style(DEFAULT_STYLE.clone())),
        Line::from(Span::raw("  (at your option) any later version...").style(DEFAULT_STYLE.clone())),
        Line::from(Span::raw("").style(DEFAULT_STYLE.clone())),
        Line::from(Span::styled("--- Keybindings ---", ALT_FG_STYLE.clone().add_modifier(Modifier::BOLD))),
        Line::from(Span::raw("").style(DEFAULT_STYLE.clone())),
        // Example binding line
        Line::from(vec![
            Span::styled("  [q]      ", KEY_STYLE.clone()), 
            Span::raw("Quit application").style(DEFAULT_STYLE.clone())
        ]),
        // ... Apply styles similarly to other help lines ...
    ];
}

// --- Helper Functions ---

/// Creates a standard block for dialogs.
fn create_dialog_block(title_text: &str, bottom_title: Title<'static>, border_style: Style) -> Block<'static> {
    let title = Title::from(Span::styled(format!(" {} ", title_text), TITLE_STYLE.clone()))
        .alignment(Alignment::Left)
        .position(Position::Top);
    Block::default()
        .title(title)
        .title(bottom_title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(DEFAULT_STYLE.clone()) // Set default background/foreground for the block area
}

/// Renders a scroll indicator `[current/total]` at the top-right of the area.
fn render_scroll_indicator(
    frame: &mut Frame,
    area: Rect,
    current_line: usize, // 1-based for display
    total_lines: usize,
    style: Style, // Pass style instead of just color
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
        let scroll_indicator = Paragraph::new(scroll_info).style(style);
        frame.render_widget(scroll_indicator, scroll_rect);
    }
}

// --- Main UI Rendering ---

/// Render main app UI frame
pub fn ui(frame: &mut Frame, app_state: &mut AppState) {
    // New Layout: Top Tabs (3), Middle (Fill), Bottom Bar (1)
    let main_chunks = Layout::vertical([
        Constraint::Length(3), // Top: Log Tabs
        Constraint::Min(0),    // Middle: Events + Preview
        Constraint::Length(1), // Bottom: Help Bar
    ])
    .split(frame.size());

    let top_area = main_chunks[0];
    let middle_area = main_chunks[1];
    let bottom_area = main_chunks[2];

    // Split Middle: Events (Left, 65%), Preview (Right, 35%)
    let middle_chunks = Layout::horizontal([
        Constraint::Percentage(65),
        Constraint::Percentage(35),
    ])
    .split(middle_area);

    let events_area = middle_chunks[0];
    let preview_area = middle_chunks[1];

    // Render Components
    render_log_tabs(frame, app_state, top_area);
    render_event_table(frame, app_state, events_area);
    render_preview_panel(frame, app_state, preview_area);
    render_bottom_bar(frame, app_state, bottom_area);

    // Render dialogs if they're visible (order matters for layering)
    render_status_dialog(frame, app_state); // Render status/confirm first
    render_event_details_dialog(frame, app_state);
    render_filter_dialog(frame, app_state);
    render_help_dialog(frame, app_state);
    render_search_bar(frame, app_state); // Search bar on top
}

// --- Panel Rendering ---

// NEW function to render log tabs horizontally
fn render_log_tabs(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let version_string = format!("v{}", VERSION);
    let version_title = Title::from(Span::styled(version_string, DARK_GRAY_FG_STYLE.clone()))
        .alignment(Alignment::Right)
        .position(Position::Top);

    let title = Title::from(Span::styled(" Event Commander ", TITLE_STYLE.clone()))
        .alignment(Alignment::Left)
        .position(Position::Top);

    let block = Block::default()
        .title(title) // Use styled title
        .title(version_title)
        .borders(Borders::ALL)
        .border_style(BORDER_STYLE.clone())
        .style(DEFAULT_STYLE.clone()); // Set background for the block area
    frame.render_widget(block.clone(), area);

    let inner_area = block.inner(area);
    if inner_area.height < 1 {
        return;
    }

    let mut tab_spans: Vec<Span> = Vec::new();
    tab_spans.push(Span::styled(" Event Logs: ", ALT_FG_STYLE.clone())); // Dark gray prefix

    for (i, log_name) in LOG_NAMES.iter().enumerate() {
        let key_hint = format!("[{}]", i + 1);
        let is_selected = app_state.selected_log_index == i;

        let style = if is_selected {
            SELECTION_STYLE.clone()
        } else {
            DEFAULT_STYLE.clone() // Use default style for non-selected tabs
        };

        tab_spans.push(Span::styled(key_hint, KEY_STYLE.clone())); // Keys use SELECTION_STYLE
        tab_spans.push(Span::raw(":").style(style.clone())); // Use tab style for colon
        tab_spans.push(Span::styled(log_name.to_string(), style));
        tab_spans.push(Span::raw("  ").style(DEFAULT_STYLE.clone())); // Spacer
    }

    let tabs_line = Line::from(tab_spans).alignment(Alignment::Left);
    let tabs_paragraph = Paragraph::new(tabs_line)
        .block(Block::default()) // No inner block needed
        .style(DEFAULT_STYLE.clone()); // Ensure background is set

    let v_margin = inner_area.height.saturating_sub(1) / 2;
    let tabs_render_area = Rect { y: inner_area.y + v_margin, height: 1, ..inner_area };
    frame.render_widget(tabs_paragraph, tabs_render_area);
}

fn render_event_table(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Events;
    let border_style = if is_focused { BORDER_STYLE.clone().fg(WHITE) } else { BORDER_STYLE.clone() }; // Brighter border when focused

    let event_table_title = Title::from(Span::styled(
        format!(" Events: {} ", app_state.selected_log_name),
        TITLE_STYLE.clone()
    )).alignment(Alignment::Left).position(Position::Top);

    let event_table_block = Block::default()
        .title(event_table_title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(DEFAULT_STYLE.clone());

    if app_state.events.is_empty() {
        let message = if app_state.active_filter.is_some() {
            "No events found matching filter criteria"
        } else {
            "No events found"
        };

        let inner_area = event_table_block.inner(area);
        let vertical_layout = Layout::vertical([
            Constraint::Percentage(40), // Top space
            Constraint::Length(3),      // Message height
            Constraint::Percentage(40), // Bottom space
        ]).split(inner_area);

        frame.render_widget(event_table_block, area);

        let centered_text = Paragraph::new(message)
            .style(DEFAULT_STYLE.clone().fg(GRAY).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);

        frame.render_widget(centered_text, vertical_layout[1]);
    } else {
        let event_rows: Vec<Row> = app_state.events.iter().map(|event| {
            let level_style = match event.level.as_str() {
                "Warning" => WARN_FG_STYLE.clone(),
                "Error" | "Critical" => ERROR_FG_STYLE.clone(),
                _ => DEFAULT_STYLE.clone(),
            };
            Row::new([
                Cell::from(event.level.clone()).style(level_style),
                Cell::from(event.datetime.clone()),
                Cell::from(event.source.clone()),
                Cell::from(event.id.clone()),
            ]).style(DEFAULT_STYLE.clone()) // Style for the row
        }).collect();

        let sort_indicator = if app_state.sort_descending { " ↓" } else { " ↑" };
        let datetime_header = format!("Date and Time{}", sort_indicator);
        let header_cells = [
            Cell::from("Level").style(HEADER_STYLE.clone()),
            Cell::from(datetime_header).style(HEADER_STYLE.clone()),
            Cell::from("Source").style(HEADER_STYLE.clone()),
            Cell::from("Event ID").style(HEADER_STYLE.clone()),
        ];
        let header = Row::new(header_cells).style(HEADER_ROW_STYLE.clone()).height(1);

        let widths = [
            Constraint::Length(11),
            Constraint::Length(22),
            Constraint::Percentage(60),
            Constraint::Length(10),
        ];

        let event_table = Table::new(event_rows, widths)
            .header(header)
            .block(event_table_block)
            .highlight_style(SELECTION_STYLE.clone()) // Use theme selection style
            .highlight_symbol(" ") // NC often just uses highlight bg
            .column_spacing(1)
            .style(DEFAULT_STYLE.clone()); // Base style for the table

        frame.render_stateful_widget(event_table, area, &mut app_state.table_state);
    }
}

fn render_preview_panel(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let is_focused = app_state.focus == PanelFocus::Preview;
    let border_style = if is_focused { BORDER_STYLE.clone().fg(WHITE) } else { BORDER_STYLE.clone() };

    let title = Title::from(Span::styled(" Event Message Preview ", TITLE_STYLE.clone()))
        .alignment(Alignment::Left)
        .position(Position::Top);

    let preview_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(DEFAULT_STYLE.clone());

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
    let available_height = area.height.saturating_sub(2); // Account for borders
    app_state.preview_scroll = app_state
        .preview_scroll
        .min(message_lines.saturating_sub(available_height.max(1)));

    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        .scroll((app_state.preview_scroll, 0))
        .style(DEFAULT_STYLE.clone()); // Apply default style

    frame.render_widget(preview_paragraph, area);
}

// --- Dialog Rendering ---

fn render_event_details_dialog(frame: &mut Frame, app_state: &mut AppState) {
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            let dialog_width = 70.min(frame.size().width.saturating_sub(4));
            let dialog_height = 20.min(frame.size().height.saturating_sub(4));
            let dialog_area = helpers::centered_fixed_rect(dialog_width, dialog_height, frame.size());

            frame.render_widget(Clear, dialog_area);

            let view_mode_suffix = match event_details.view_mode {
                DetailsViewMode::Formatted => " (Formatted)",
                DetailsViewMode::RawXml => " (Raw XML)",
            };
            let dialog_title_text = format!("{}{}", event_details.title, view_mode_suffix);

            let dialog_block = create_dialog_block(
                &dialog_title_text,
                EVENT_DETAILS_HELP_TITLE.clone(),
                BORDER_STYLE.clone(),
            );
            frame.render_widget(dialog_block.clone(), dialog_area);
            let content_area = dialog_block.inner(dialog_area);

            event_details.current_visible_height = (content_area.height as usize).max(1);
            let visible_height = event_details.current_visible_height;
            let content = event_details.current_content();
            let content_lines: Vec<Line> = content.lines().map(|l| Line::from(Span::raw(l).style(DEFAULT_STYLE.clone()))).collect();
            let total_lines = content_lines.len();

            let start_line = event_details.scroll_position.min(total_lines.saturating_sub(1));
            let wrap_behavior = match event_details.view_mode {
                DetailsViewMode::Formatted => Wrap { trim: true },
                DetailsViewMode::RawXml => Wrap { trim: false },
            };

            let content_paragraph = Paragraph::new(Text::from(content_lines))
                .wrap(wrap_behavior)
                .style(DEFAULT_STYLE.clone())
                .scroll((start_line as u16, 0));

            frame.render_widget(content_paragraph, content_area);

            render_scroll_indicator(
                frame, content_area, start_line + 1, total_lines,
                TITLE_STYLE.clone()
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

            let border_color = if status_dialog.is_error { THEME_ERROR_FG } else { GREEN };
            let border_style = Style::default().fg(border_color);

            let dialog_block = create_dialog_block(
                &status_dialog.title,
                STATUS_DISMISS_TITLE.clone(),
                border_style,
            );
            frame.render_widget(dialog_block.clone(), dialog_area);
            let content_area = dialog_block.inner(dialog_area);

            let message_paragraph = Paragraph::new(status_dialog.message.clone())
                .wrap(Wrap { trim: true })
                .style(DEFAULT_STYLE.clone());
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

        let search_block = Block::default()
            .title(SEARCH_BAR_TITLE.clone())
            .borders(Borders::ALL)
            .border_style(BORDER_STYLE.clone())
            .style(DEFAULT_STYLE.clone());

        let search_text = format!(" {}_", app_state.search_term);
        let search_paragraph = Paragraph::new(search_text)
            .block(search_block.clone())
            .style(INPUT_FOCUSED_STYLE.clone());

        frame.render_widget(Clear, search_area);
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
            + list_height
            + EVENT_ID_LABEL_HEIGHT
            + EVENT_ID_INPUT_HEIGHT
            + LEVEL_SELECT_HEIGHT
            + BUTTON_SPACER_HEIGHT
            + BUTTON_ROW_HEIGHT;

        let dialog_height = total_inner_content_height + INNER_MARGIN_HEIGHT + BORDERS_HEIGHT;

        let dialog_area = helpers::centered_fixed_rect(
            dialog_width,
            dialog_height.min(frame.size().height),
            frame.size(),
        );
        frame.render_widget(Clear, dialog_area);

        let dialog_block = create_dialog_block(
            "Filter Events",
            FILTER_CANCEL_TITLE.clone(),
            BORDER_STYLE.clone(),
        );
        let inner_area = dialog_block.inner(dialog_area);
        frame.render_widget(dialog_block, dialog_area);

        let constraints = vec![
            Constraint::Length(SOURCE_LABEL_HEIGHT),
            Constraint::Length(SOURCE_INPUT_HEIGHT),
            Constraint::Length(list_height),
            Constraint::Length(EVENT_ID_LABEL_HEIGHT),
            Constraint::Length(EVENT_ID_INPUT_HEIGHT),
            Constraint::Length(LEVEL_SELECT_HEIGHT),
            Constraint::Length(BUTTON_SPACER_HEIGHT),
            Constraint::Length(BUTTON_ROW_HEIGHT),
        ];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(constraints)
            .split(inner_area);

        let mut chunk_index = 0;
        let num_chunks = chunks.len();

        if chunk_index < num_chunks { frame.render_widget(Paragraph::new("Source:").style(DEFAULT_STYLE.clone()), chunks[chunk_index]); chunk_index += 1; }
        let source_style = if is_source_focused { INPUT_FOCUSED_STYLE.clone() } else { INPUT_UNFOCUSED_STYLE.clone() };
        let source_input_display = if is_source_focused {
            format!("{}_", app_state.filter_dialog_source_input)
        } else if app_state.filter_dialog_source_input.is_empty() {
            "[Any Source]".to_string()
        } else {
            app_state.filter_dialog_source_input.clone()
        };
        if chunk_index < num_chunks { frame.render_widget(Paragraph::new(source_input_display).style(source_style), chunks[chunk_index]); chunk_index += 1; }

        if chunk_index < num_chunks {
             if list_visible {
                 let list_items: Vec<ListItem> = app_state.filter_dialog_filtered_sources.iter()
                    .map(|(_, name)| ListItem::new(name.clone()))
                    .collect();
                 let list = List::new(list_items)
                    .highlight_style(SELECTION_STYLE.clone())
                    .style(DEFAULT_STYLE.clone())
                    .highlight_symbol(" ");
                 let mut list_state = ListState::default();
                 list_state.select(app_state.filter_dialog_filtered_source_selection);
                 frame.render_stateful_widget(list, chunks[chunk_index], &mut list_state);
             }
             chunk_index += 1;
        }

        if chunk_index < num_chunks { frame.render_widget(Paragraph::new("Event ID:").style(DEFAULT_STYLE.clone()), chunks[chunk_index]); chunk_index += 1; }
        let is_eventid_focused = app_state.filter_dialog_focus == FilterFieldFocus::EventId;
        let event_id_input_style = if is_eventid_focused { INPUT_FOCUSED_STYLE.clone() } else { INPUT_UNFOCUSED_STYLE.clone() };
        let event_id_text = if is_eventid_focused {
            format!("{}_", app_state.filter_dialog_event_id)
        } else {
            app_state.filter_dialog_event_id.clone()
        };
        if chunk_index < num_chunks { frame.render_widget(Paragraph::new(event_id_text).style(event_id_input_style), chunks[chunk_index]); chunk_index += 1; }

        if chunk_index < num_chunks { frame.render_widget(Paragraph::new("Level:").style(DEFAULT_STYLE.clone()), chunks[chunk_index]); chunk_index += 1; }
        let is_level_focused = app_state.filter_dialog_focus == FilterFieldFocus::Level;
        let level_style = if is_level_focused {
             SELECTION_STYLE.clone()
        } else { DEFAULT_STYLE.clone() };
        let level_text = Line::from(vec![
            Span::raw("Level: ").style(DEFAULT_STYLE.clone()),
            Span::styled("< ", ALT_FG_STYLE.clone()),
            Span::styled(app_state.filter_dialog_level.display_name(), level_style),
            Span::styled(" >", ALT_FG_STYLE.clone()),
        ]);
        if chunk_index < num_chunks { frame.render_widget(Paragraph::new(level_text), chunks[chunk_index]); chunk_index += 1; }

        if chunk_index < num_chunks { chunk_index += 1; }

        if chunk_index < num_chunks {
            let apply_style = if app_state.filter_dialog_focus == FilterFieldFocus::Apply {
                SELECTION_STYLE.clone()
            } else { DEFAULT_STYLE.clone() };
            let clear_style = if app_state.filter_dialog_focus == FilterFieldFocus::Clear {
                SELECTION_STYLE.clone()
            } else { DEFAULT_STYLE.clone() };
            let apply_text = Span::styled(" [ Apply ] ", apply_style);
            let clear_text = Span::styled(" [ Clear ] ", clear_style);
            let button_line = Line::from(vec![apply_text, Span::raw(" "), clear_text])
                .alignment(Alignment::Center);
            frame.render_widget(Paragraph::new(button_line), chunks[chunk_index]);
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
            BORDER_STYLE.clone().fg(THEME_ALT_FG),
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
            .style(DEFAULT_STYLE.clone())
            .scroll((current_scroll as u16, 0));

        frame.render_widget(help_paragraph, content_area);

        render_scroll_indicator(
            frame, content_area, current_scroll + 1, total_lines,
            TITLE_STYLE.clone().fg(THEME_ALT_FG)
        );
    }
}

// --- Bottom Bar ---
fn render_bottom_bar(frame: &mut Frame, app_state: &mut AppState, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();

    spans.push(Span::styled("[q]", KEY_STYLE.clone())); spans.push(Span::raw(" Quit | ").style(DEFAULT_STYLE.clone()));
    spans.push(Span::styled("[F1]", KEY_STYLE.clone())); spans.push(Span::raw(" Help | ").style(DEFAULT_STYLE.clone()));

    match app_state.focus {
        PanelFocus::Events => {
            spans.push(Span::styled("[s]", KEY_STYLE.clone())); spans.push(Span::raw(" Sort | ").style(DEFAULT_STYLE.clone()));
            spans.push(Span::styled("[l]", KEY_STYLE.clone())); spans.push(Span::raw(format!(" Lvl ({}) | ", app_state.get_current_level_name())).style(DEFAULT_STYLE.clone()));
            spans.push(Span::styled("[f]", KEY_STYLE.clone())); spans.push(Span::raw(format!(" Adv Filter ({}) | ", app_state.get_filter_status())).style(DEFAULT_STYLE.clone()));
            spans.push(Span::styled("[/]", KEY_STYLE.clone())); spans.push(Span::raw(" Search | ").style(DEFAULT_STYLE.clone()));
            if app_state.last_search_term.is_some() {
                spans.push(Span::styled("[n]", KEY_STYLE.clone())); spans.push(Span::raw(" Next | ").style(DEFAULT_STYLE.clone()));
                spans.push(Span::styled("[p]", KEY_STYLE.clone())); spans.push(Span::raw(" Prev").style(DEFAULT_STYLE.clone()));
            } else {
                if let Some(last_span) = spans.last_mut() {
                    if last_span.content == " Search | " {
                         last_span.content = " Search".into();
                    }
                }
            }
        }
        PanelFocus::Preview => {
        }
    }

    if app_state.is_loading {
        if !spans.is_empty() && spans.last().map_or(false, |s| !s.content.ends_with(' ')) {
             spans.push(Span::raw(" | ").style(DEFAULT_STYLE.clone()));
        }
        spans.push(Span::styled("Loading...", ALT_FG_STYLE.clone()));
    }

    let line = Line::from(spans).alignment(Alignment::Left);
    frame.render_widget(Paragraph::new(line).style(DEFAULT_STYLE.clone()), area);
} 