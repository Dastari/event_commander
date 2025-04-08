use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState},
};
use std::{
    error::Error,
    io::{self, stdout, Stdout},
    time::Duration,
    vec,
};
use windows::{
    core::PCWSTR,
    Win32::Foundation::{GetLastError, ERROR_NO_MORE_ITEMS},
    Win32::System::EventLog::{EvtQuery, EvtNext, EvtRender, EvtClose, EvtRenderEventXml, EVT_HANDLE, EvtQueryChannelPath, EvtQueryReverseDirection},
};

const EVENT_BATCH_SIZE: usize = 100;
const LOG_NAMES: [&str; 5] = [
    "Application",
    "System",
    "Security",
    "Setup",
    "ForwardedEvents",
];

fn to_wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn render_event_xml(event_handle: EVT_HANDLE) -> Option<String> {
    unsafe {
        let mut buffer_used: u32 = 0;
        let mut property_count: u32 = 0;
        let _ = EvtRender(None, event_handle, EvtRenderEventXml.0, 0, None, &mut buffer_used, &mut property_count);
        if buffer_used == 0 {
            return None;
        }
        let mut buffer: Vec<u16> = vec![0; buffer_used as usize];
        if EvtRender(None, event_handle, EvtRenderEventXml.0, buffer_used, Some(buffer.as_mut_ptr() as *mut _), &mut buffer_used, &mut property_count).is_ok() {
            let xml = String::from_utf16_lossy(&buffer);
            Some(xml)
        } else {
            None
        }
    }
}

// Helper function to extract attribute values regardless of quote type
fn find_attribute_value<'a>(xml: &'a str, attribute_name: &str) -> Option<&'a str> {
    if let Some(start_pos) = xml.find(&format!("{}='", attribute_name)) {
        let sub = &xml[start_pos + attribute_name.len() + 2..];
        if let Some(end_pos) = sub.find('\'') {
            return Some(&sub[..end_pos]);
        }
    } else if let Some(start_pos) = xml.find(&format!("{}=\"", attribute_name)) {
        let sub = &xml[start_pos + attribute_name.len() + 2..];
        if let Some(end_pos) = sub.find('"') {
            return Some(&sub[..end_pos]);
        }
    }
    None
}

fn parse_event_xml(xml: &str) -> DisplayEvent {
    let source = find_attribute_value(xml, "Provider Name")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<unknown provider>".to_string());

    let id = if let Some(start) = xml.find("<EventID>") {
        let sub = &xml[start + "<EventID>".len()..];
        if let Some(end) = sub.find("</EventID>") {
            sub[..end].trim().to_string()
        } else {
            "0".to_string()
        }
    } else {
        "0".to_string()
    };

    let level_raw = if let Some(start) = xml.find("<Level>") {
        let sub = &xml[start + "<Level>".len()..];
        if let Some(end) = sub.find("</Level>") {
            sub[..end].trim().to_string()
        } else {
            "0".to_string()
        }
    } else {
        "0".to_string()
    };
    let level = match level_raw.as_str() {
        "1" => "Critical".to_string(),
        "2" => "Error".to_string(),
        "3" => "Warning".to_string(),
        "4" => "Information".to_string(),
        "5" => "Verbose".to_string(),
        other => format!("Unknown({})", other),
    };

    let datetime = find_attribute_value(xml, "TimeCreated SystemTime")
        .map(|time_str| {
            match chrono::DateTime::parse_from_rfc3339(time_str) {
                Ok(dt) => dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string(),
                Err(_) => time_str.to_string(), // Fallback to raw string if parsing fails
            }
        })
        .unwrap_or_else(|| "".to_string());

    DisplayEvent {
        level,
        datetime,
        source,
        id,
        raw_data: xml.to_string(),
    }
}

#[derive(Clone, Debug)]
struct DisplayEvent {
    level: String,
    datetime: String,
    source: String,
    id: String,
    raw_data: String,
}

#[derive(Debug, Clone)]
struct ErrorDialog {
    title: String,
    message: String,
    visible: bool,
}

#[derive(Debug, Clone)]
struct EventDetailsDialog {
    title: String,
    content: String,
    visible: bool,
    scroll_position: usize,
}

impl ErrorDialog {
    fn new(title: &str, message: &str) -> Self {
        ErrorDialog {
            title: title.to_string(),
            message: message.to_string(),
            visible: true,
        }
    }
    fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl EventDetailsDialog {
    fn new(title: &str, content: &str) -> Self {
        EventDetailsDialog {
            title: title.to_string(),
            content: content.to_string(),
            visible: true,
            scroll_position: 0,
        }
    }
    fn dismiss(&mut self) {
        self.visible = false;
    }
    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }
    fn scroll_down(&mut self, max_lines: usize) {
        let content_lines = self.content.lines().count();
        if content_lines > max_lines && self.scroll_position < content_lines - max_lines {
            self.scroll_position += 1;
        }
    }
    fn page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(10);
    }
    fn page_down(&mut self, max_lines: usize) {
        let content_lines = self.content.lines().count();
        if content_lines > max_lines {
            self.scroll_position = (self.scroll_position + 10).min(content_lines - max_lines);
        }
    }
}

#[derive(PartialEq, Debug)]
enum PanelFocus {
    Logs,
    Events,
}

struct AppState {
    focus: PanelFocus,
    selected_log_index: usize,
    selected_log_name: String,
    events: Vec<DisplayEvent>,
    table_state: TableState,
    error_dialog: Option<ErrorDialog>,
    event_details_dialog: Option<EventDetailsDialog>,
    log_file: Option<std::fs::File>,
}

impl AppState {
    fn new() -> Self {
        let log_file = std::fs::OpenOptions::new().create(true).write(true).append(true).open("event_commander.log").ok();
        AppState {
            focus: PanelFocus::Logs,
            selected_log_index: 0,
            selected_log_name: "".to_string(),
            events: Vec::new(),
            table_state: TableState::default(),
            error_dialog: None,
            event_details_dialog: None,
            log_file,
        }
    }
    fn log(&mut self, message: &str) {
        if let Some(file) = &mut self.log_file {
            use std::io::Write;
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let log_entry = format!("[{}] {}\n", timestamp, message);
            let _ = file.write_all(log_entry.as_bytes());
            let _ = file.flush();
        }
    }
    fn show_error(&mut self, title: &str, message: &str) {
        self.error_dialog = Some(ErrorDialog::new(title, message));
        self.log(&format!("ERROR - {}: {}", title, message));
    }
    fn show_event_details(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(event) = self.events.get(selected) {
                let title = format!("Event Details: {} ({})", event.source, event.id);
                self.event_details_dialog = Some(EventDetailsDialog::new(&title, &event.raw_data));
                self.log(&format!("Showing details for event ID {}", event.id));
            }
        }
    }
    #[cfg(target_os = "windows")]
    fn load_events_for_selected_log(&mut self) {
        self.selected_log_name = LOG_NAMES.get(self.selected_log_index).map(|s| s.to_string()).unwrap_or_else(|| "".to_string());
        self.events.clear();
        self.table_state = TableState::default();
        if self.selected_log_name.is_empty() {
            self.show_error("Loading Error", "No log name selected.");
            return;
        }
        self.log(&format!("Loading events from {}", self.selected_log_name));
        let channel_wide = to_wide_string(&self.selected_log_name);
        let query_str_wide = to_wide_string("*");
        unsafe {
            let flags = EvtQueryChannelPath.0 | EvtQueryReverseDirection.0;
            let query_handle = EvtQuery(None, PCWSTR::from_raw(channel_wide.as_ptr()), PCWSTR::from_raw(query_str_wide.as_ptr()), flags);
            if query_handle.is_err() {
                self.show_error("Query Error", &format!("Failed to query log '{}'", self.selected_log_name));
                return;
            }
            let query_handle = query_handle.unwrap();
            loop {
                let mut events: Vec<EVT_HANDLE> = vec![EVT_HANDLE::default(); EVENT_BATCH_SIZE];
                let mut fetched: u32 = 0;
                let next_result = unsafe {
                    let events_slice: &mut [isize] = std::mem::transmute(events.as_mut_slice());
                    EvtNext(query_handle, events_slice, 0, 0, &mut fetched)
                };
                if !next_result.is_ok() {
                    let error = GetLastError().0;
                    if error == ERROR_NO_MORE_ITEMS.0 {
                        break;
                    } else {
                        self.show_error("Reading Error", &format!("Error reading event log '{}': WIN32_ERROR({})", self.selected_log_name, error));
                        break;
                    }
                }
                for i in 0..(fetched as usize) {
                    let event_handle = events[i];
                    if let Some(xml) = render_event_xml(event_handle) {
                        let display_event = parse_event_xml(&xml);
                        self.events.push(display_event);
                    }
                    let _ = EvtClose(event_handle);
                    if self.events.len() >= EVENT_BATCH_SIZE {
                        break;
                    }
                }
                if self.events.len() >= EVENT_BATCH_SIZE || fetched == 0 {
                    break;
                }
            }
            let _ = EvtClose(query_handle);
        }

        if !self.events.is_empty() {
            self.table_state.select(Some(0));
            self.log(&format!("Loaded {} events from {}", self.events.len(), self.selected_log_name));
        } else {
            self.table_state.select(None);
            self.show_error("Loading Error", &format!("No events found in {}", self.selected_log_name));
        }
    }
    fn next_log(&mut self) {
        if self.selected_log_index < LOG_NAMES.len() - 1 {
            self.selected_log_index += 1;
        }
    }
    fn previous_log(&mut self) {
        self.selected_log_index = self.selected_log_index.saturating_sub(1);
    }
    fn scroll_down(&mut self) {
        if self.events.is_empty() {
            self.table_state.select(None);
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.events.len().saturating_sub(1) {
                    i
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }
    fn scroll_up(&mut self) {
        if self.events.is_empty() {
            self.table_state.select(None);
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }
    fn page_down(&mut self) {
        if self.events.is_empty() {
            self.table_state.select(None);
            return;
        }
        let page_size = 10;
        let i = match self.table_state.selected() {
            Some(i) => (i + page_size).min(self.events.len().saturating_sub(1)),
            None => 0,
        };
        self.table_state.select(Some(i));
    }
    fn page_up(&mut self) {
        if self.events.is_empty() {
            self.table_state.select(None);
            return;
        }
        let page_size = 10;
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(page_size),
            None => 0,
        };
        self.table_state.select(Some(i));
    }
    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Logs => PanelFocus::Events,
            PanelFocus::Events => PanelFocus::Logs,
        };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = init_terminal()?;
    let mut app_state = AppState::new();
    loop {
        terminal.draw(|frame| ui(frame, &mut app_state))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_press(key, &mut app_state);
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            }
        }
    }
    restore_terminal()?;
    Ok(())
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn ui(frame: &mut Frame, app_state: &mut AppState) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(frame.size());
    let log_items: Vec<ListItem> = LOG_NAMES.iter().map(|&name| ListItem::new(name)).collect();
    let log_list_block = Block::default().title("Windows Logs").borders(Borders::ALL).border_style(Style::default().fg(if app_state.focus == PanelFocus::Logs { Color::Cyan } else { Color::White }));
    let log_list = List::new(log_items)
        .block(log_list_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(if app_state.focus == PanelFocus::Logs { Color::Blue } else { Color::DarkGray }))
        .highlight_symbol("> ");
    let mut log_list_state = ListState::default();
    log_list_state.select(Some(app_state.selected_log_index));
    frame.render_stateful_widget(log_list, main_layout[0], &mut log_list_state);
    let event_rows: Vec<Row> = app_state.events.iter().map(|event| Row::new(vec![Cell::from(event.level.clone()), Cell::from(event.datetime.clone()), Cell::from(event.source.clone()), Cell::from(event.id.clone())])).collect();
    let header_cells = ["Level", "Date and Time", "Source", "Event ID"].iter().map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::DarkGray)).height(1).bottom_margin(0);
    let widths = [Constraint::Length(10), Constraint::Length(20), Constraint::Percentage(60), Constraint::Length(10)];
    let event_table_block = Block::default().title(format!("Events: {}", app_state.selected_log_name)).borders(Borders::ALL).border_style(Style::default().fg(if app_state.focus == PanelFocus::Events { Color::Cyan } else { Color::White }));
    let event_table = Table::new(event_rows, widths)
        .header(header)
        .block(event_table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .column_spacing(1);
    frame.render_stateful_widget(event_table, main_layout[1], &mut app_state.table_state);
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            let dialog_width = 70.min(frame.size().width.saturating_sub(4));
            let dialog_height = 20.min(frame.size().height.saturating_sub(4));
            let dialog_area = Rect::new((frame.size().width - dialog_width) / 2, (frame.size().height - dialog_height) / 2, dialog_width, dialog_height);
            frame.render_widget(Clear, dialog_area);
            let dialog_block = Block::default().title(event_details.title.clone()).borders(Borders::ALL).border_style(Style::default().fg(Color::Blue));
            let dialog_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .margin(1)
                .split(dialog_area);
            frame.render_widget(dialog_block, dialog_area);
            let content_lines: Vec<&str> = event_details.content.lines().collect();
            let visible_height = dialog_layout[0].height as usize;
            let start_line = event_details.scroll_position.min(content_lines.len().saturating_sub(1));
            let end_line = (start_line + visible_height).min(content_lines.len());
            let visible_content = content_lines[start_line..end_line].join("\n");
            let scroll_info = if content_lines.len() > visible_height { format!("[{}/{}]", start_line + 1, content_lines.len()) } else { "".to_string() };
            let content_paragraph = Paragraph::new(visible_content).wrap(ratatui::widgets::Wrap { trim: false }).style(Style::default().fg(Color::White));
            frame.render_widget(content_paragraph, dialog_layout[0]);
            if !scroll_info.is_empty() {
                let scroll_rect = Rect::new(dialog_area.right() - scroll_info.len() as u16 - 2, dialog_area.y + 1, scroll_info.len() as u16, 1);
                let scroll_indicator = Paragraph::new(scroll_info).style(Style::default().fg(Color::Blue));
                frame.render_widget(scroll_indicator, scroll_rect);
            }
            let dismiss_button = Paragraph::new("  [Dismiss (Esc)]  ").block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue))).style(Style::default().fg(Color::White));
            let button_width = 20;
            let button_x = dialog_layout[1].x + (dialog_layout[1].width - button_width) / 2;
            let button_area = Rect::new(button_x, dialog_layout[1].y, button_width, 3);
            frame.render_widget(dismiss_button, button_area);
        }
    }
    if let Some(error_dialog) = &app_state.error_dialog {
        if error_dialog.visible {
            let dialog_width = 60.min(frame.size().width - 4);
            let dialog_height = 10.min(frame.size().height - 4);
            let dialog_area = Rect::new((frame.size().width - dialog_width) / 2, (frame.size().height - dialog_height) / 2, dialog_width, dialog_height);
            frame.render_widget(Clear, dialog_area);
            let dialog_block = Block::default().title(error_dialog.title.clone()).borders(Borders::ALL).border_style(Style::default().fg(Color::Red));
            let dialog_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .margin(1)
                .split(dialog_area);
            frame.render_widget(dialog_block, dialog_area);
            let message_paragraph = Paragraph::new(error_dialog.message.clone()).wrap(ratatui::widgets::Wrap { trim: true }).style(Style::default().fg(Color::White));
            frame.render_widget(message_paragraph, dialog_layout[0]);
            let dismiss_button = Paragraph::new("  [Dismiss (Enter)]  ").block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Red))).style(Style::default().fg(Color::White));
            let button_width = 20;
            let button_x = dialog_layout[1].x + (dialog_layout[1].width - button_width) / 2;
            let button_area = Rect::new(button_x, dialog_layout[1].y, button_width, 3);
            frame.render_widget(dismiss_button, button_area);
        }
    }
}

fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) {
    if let Some(error_dialog) = &mut app_state.error_dialog {
        if error_dialog.visible {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                error_dialog.dismiss();
                app_state.log("Dismissed error dialog");
            }
            return;
        }
    }
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            match key.code {
                KeyCode::Esc => {
                    event_details.dismiss();
                    app_state.log("Dismissed event details dialog");
                }
                KeyCode::Up => event_details.scroll_up(),
                KeyCode::Down => event_details.scroll_down(18),
                KeyCode::PageUp => event_details.page_up(),
                KeyCode::PageDown => event_details.page_down(18),
                _ => {}
            }
            return;
        }
    }
    match app_state.focus {
        PanelFocus::Logs => match key.code {
            KeyCode::Char('q') => return,
            KeyCode::Up => app_state.previous_log(),
            KeyCode::Down => app_state.next_log(),
            KeyCode::Right | KeyCode::Tab => {
                let current_log_name = LOG_NAMES.get(app_state.selected_log_index).map(|s| s.to_string()).unwrap_or_default();
                if app_state.events.is_empty() || current_log_name != app_state.selected_log_name {
                    #[cfg(target_os = "windows")]
                    app_state.load_events_for_selected_log();
                }
                app_state.switch_focus();
            }
            KeyCode::Enter => {
                #[cfg(target_os = "windows")]
                app_state.load_events_for_selected_log();
                app_state.switch_focus();
            }
            _ => {}
        },
        PanelFocus::Events => match key.code {
            KeyCode::Char('q') => return,
            KeyCode::Up => app_state.scroll_up(),
            KeyCode::Down => app_state.scroll_down(),
            KeyCode::PageUp => app_state.page_up(),
            KeyCode::PageDown => app_state.page_down(),
            KeyCode::Enter => app_state.show_event_details(),
            KeyCode::Left => app_state.switch_focus(),
            KeyCode::BackTab => app_state.switch_focus(),
            KeyCode::Tab => app_state.switch_focus(),
            _ => {}
        },
    }
}
