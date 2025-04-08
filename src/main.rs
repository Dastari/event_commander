use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap},
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
    let mut source = find_attribute_value(xml, "Provider Name")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<unknown provider>".to_string());

    // Remove "Microsoft-Windows-" prefix if present
    if source.starts_with("Microsoft-Windows-") {
        source = source.trim_start_matches("Microsoft-Windows-").to_string();
    }

    // Extract EventID from within the System block to handle attributes
    let id = extract_tag_content(xml, "System")
        .and_then(|system_xml| extract_tag_content(system_xml, "EventID"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "0".to_string());

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

    // Extract message based on event type or fallback
    let message = if let Some(event_data_xml) = extract_tag_content(xml, "EventData") {
        if source == "Windows Error Reporting" && id == "1001" {
            format_wer_event_data(event_data_xml)
        } else {
            // Fallback: Format as key-value pairs
            format_simple_xml_section(event_data_xml)
        }
    } else {
        "<No EventData found>".to_string()
    };

    DisplayEvent {
        level,
        datetime,
        source,
        id,
        message, // Use the formatted message
        raw_data: xml.to_string(),
    }
}

#[derive(Clone, Debug)]
struct DisplayEvent {
    level: String,
    datetime: String,
    source: String,
    id: String,
    message: String, // Add message field
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

#[derive(PartialEq, Debug, Clone, Copy)]
enum PanelFocus {
    Logs,
    Events,
    Preview,
}

struct AppState {
    focus: PanelFocus,
    selected_log_index: usize,
    selected_log_name: String,
    events: Vec<DisplayEvent>,
    table_state: TableState,
    preview_scroll: u16, // Scroll position for preview pane
    error_dialog: Option<ErrorDialog>,
    event_details_dialog: Option<EventDetailsDialog>,
    log_file: Option<std::fs::File>,
    #[cfg(target_os = "windows")]
    query_handle: Option<EVT_HANDLE>,
    is_loading: bool,
    no_more_events: bool,
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
            preview_scroll: 0, // Initialize scroll
            error_dialog: None,
            event_details_dialog: None,
            log_file,
            #[cfg(target_os = "windows")]
            query_handle: None,
            is_loading: false,
            no_more_events: false,
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

                // Attempt to parse and format the XML nicely
                let mut formatted_content = String::new();
                formatted_content.push_str(&format!(
                    "Level:       {}
DateTime:    {}
Source:      {}
Event ID:    {}\n",
                    event.level, event.datetime, event.source, event.id
                ));

                if let Some(system_xml) = extract_tag_content(&event.raw_data, "System") {
                    formatted_content.push_str("\n--- System ---\n");
                    formatted_content.push_str(&format_simple_xml_section(system_xml));
                    formatted_content.push('\n');
                }

                if let Some(event_data_xml) = extract_tag_content(&event.raw_data, "EventData") {
                    formatted_content.push_str("\n--- Event Data ---\n");
                    formatted_content.push_str(&format_simple_xml_section(event_data_xml));
                    formatted_content.push('\n');
                }

                // Fallback or append remaining raw data if needed (optional)
                // if formatted_content.len() < 100 { // Arbitrary threshold
                //     formatted_content.push_str("\n--- Raw XML ---\n");
                //     formatted_content.push_str(&event.raw_data);
                // }

                self.event_details_dialog = Some(EventDetailsDialog::new(&title, &formatted_content));
                self.log(&format!("Showing formatted details for event ID {}", event.id));
            }
        }
    }
    #[cfg(target_os = "windows")]
    fn start_or_continue_log_load(&mut self, initial_load: bool) {
        if self.is_loading {
            self.log("Load requested but already in progress.");
            return;
        }

        if !initial_load && self.no_more_events {
            self.log("Load requested but no more events to fetch.");
            return;
        }

        self.is_loading = true;

        // Initial load setup
        if initial_load {
            self.log(&format!("Starting initial load for log: {}", self.selected_log_name));
            // Clear previous state
            self.events.clear();
            self.table_state = TableState::default();
            self.no_more_events = false;

            // Close existing query handle if any
            if let Some(handle) = self.query_handle.take() {
                unsafe {
                    let _ = EvtClose(handle);
                }
                self.log("Closed previous query handle.");
            }

            // Get selected log name
            self.selected_log_name = LOG_NAMES.get(self.selected_log_index).map(|s| s.to_string()).unwrap_or_else(|| "".to_string());
            if self.selected_log_name.is_empty() {
                self.show_error("Loading Error", "No log name selected.");
                self.is_loading = false;
                return;
            }

            // Create new query
            let channel_wide = to_wide_string(&self.selected_log_name);
            let query_str_wide = to_wide_string("*");
            unsafe {
                let flags = EvtQueryChannelPath.0 | EvtQueryReverseDirection.0;
                match EvtQuery(None, PCWSTR::from_raw(channel_wide.as_ptr()), PCWSTR::from_raw(query_str_wide.as_ptr()), flags) {
                    Ok(handle) => {
                        self.query_handle = Some(handle);
                        self.log("Created new event query handle.");
                    }
                    Err(e) => {
                        self.show_error("Query Error", &format!("Failed to query log '{}': {}", self.selected_log_name, e));
                        self.is_loading = false;
                        return;
                    }
                }
            }
        } else {
            self.log("Continuing log load...");
        }

        // Fetch next batch (common to initial and subsequent loads)
        if let Some(query_handle) = self.query_handle {
            let _initial_event_count = self.events.len();
            let mut new_events_fetched = 0;
            unsafe {
                loop {
                    let mut events_buffer: Vec<EVT_HANDLE> = vec![EVT_HANDLE::default(); EVENT_BATCH_SIZE];
                    let mut fetched: u32 = 0;
                    let events_slice: &mut [isize] = std::mem::transmute(events_buffer.as_mut_slice());
                    let next_result = EvtNext(query_handle, events_slice, 0, 0, &mut fetched);

                    if !next_result.is_ok() {
                        let error = GetLastError().0;
                        if error == ERROR_NO_MORE_ITEMS.0 {
                            self.log("Reached end of event log.");
                            self.no_more_events = true;
                        } else {
                            self.show_error("Reading Error", &format!("Error reading event log '{}': WIN32_ERROR({})", self.selected_log_name, error));
                        }
                        break; // Exit loop on error or no more items
                    }

                    if fetched == 0 {
                        self.log("EvtNext returned 0 events, assuming end.");
                        self.no_more_events = true;
                        break;
                    }

                    for i in 0..(fetched as usize) {
                        let event_handle = events_buffer[i];
                        if let Some(xml) = render_event_xml(event_handle) {
                            let display_event = parse_event_xml(&xml);
                            self.events.push(display_event);
                            new_events_fetched += 1;
                        }
                        let _ = EvtClose(event_handle);
                    }

                    // For now, we only fetch one batch at a time per trigger
                    break;
                }
            }

            if new_events_fetched > 0 {
                self.log(&format!("Fetched {} new events (total {}).", new_events_fetched, self.events.len()));
                if initial_load && !self.events.is_empty() {
                    self.table_state.select(Some(0));
                }
            } else if initial_load {
                self.table_state.select(None);
                // Potentially show a different message than error if log is just empty
                self.log(&format!("No events found in {}", self.selected_log_name));
            }
        } else {
            self.log("Attempted to load more events, but no query handle exists.");
        }

        self.is_loading = false;
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
            self.select_event(None);
            return;
        }
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection = if current_selection >= self.events.len().saturating_sub(1) {
            current_selection
        } else {
            current_selection + 1
        };
        self.select_event(Some(new_selection));

        let load_threshold = self.events.len().saturating_sub(20);
        if new_selection >= load_threshold {
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    fn scroll_up(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.select_event(Some(i));
    }
    fn page_down(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let page_size = 10;
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection = (current_selection + page_size).min(self.events.len().saturating_sub(1));
        self.select_event(Some(new_selection));

        let load_threshold = self.events.len().saturating_sub(20);
        if new_selection >= load_threshold {
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    fn page_up(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let page_size = 10;
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(page_size),
            None => 0,
        };
        self.select_event(Some(i));
    }
    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Logs => PanelFocus::Events,
            PanelFocus::Events => PanelFocus::Preview,
            PanelFocus::Preview => PanelFocus::Logs,
        };
    }

    // Add scroll methods for preview
    fn preview_scroll_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(1);
    }

    fn preview_scroll_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(1);
    }

    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
    }

    // Reset preview scroll when table selection changes
    fn select_event(&mut self, index: Option<usize>) {
        self.table_state.select(index);
        self.reset_preview_scroll();
    }
}

// Implement Drop to ensure the query handle is closed
#[cfg(target_os = "windows")]
impl Drop for AppState {
    fn drop(&mut self) {
        if let Some(handle) = self.query_handle.take() {
            unsafe {
                let _ = EvtClose(handle);
            }
            self.log("Closed active event query handle.");
        }
    }
}

// Helper to extract text between two tags (robust, handles attributes, checks bounds)
fn extract_tag_content<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let start_tag_prefix = format!("<{}", tag);
    let end_tag = format!("</{}>", tag);

    if let Some(start_prefix_pos) = xml.find(&start_tag_prefix) {
        // Define the start for searching for the closing '>' of the start tag
        let search_start_for_gt = start_prefix_pos + start_tag_prefix.len();

        // Ensure the search start position is valid before slicing/searching
        if search_start_for_gt <= xml.len() {
            // Find the closing '>' relative to search_start_for_gt
            if let Some(start_tag_end_pos_rel) = xml[search_start_for_gt..].find('>') {
                let content_start_abs = search_start_for_gt + start_tag_end_pos_rel + 1;

                // Ensure content_start_abs is valid before slicing/searching
                if content_start_abs <= xml.len() {
                    // Find the closing tag relative to content_start_abs
                    if let Some(end_tag_pos_rel) = xml[content_start_abs..].find(&end_tag) {
                        let content_end_abs = content_start_abs + end_tag_pos_rel;

                        // Final bound check before slicing
                        if content_start_abs <= content_end_abs && content_end_abs <= xml.len() {
                            return Some(xml[content_start_abs..content_end_abs].trim());
                        }
                    }
                }
            }
        }
    }
    None // Return None if any step fails or indices are invalid
}

// Helper to format simple key-value pairs from XML sections
fn format_simple_xml_section(xml_section: &str) -> String {
    xml_section.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('<') && trimmed.ends_with('>') {
                if let Some(tag_end) = trimmed.find('>') {
                    let tag_part = &trimmed[1..tag_end];
                    if let Some(content_start) = tag_end.checked_add(1) {
                       if let Some(content_end) = trimmed.rfind("<") {
                            let content = &trimmed[content_start..content_end];
                            // Handle self-closing tags or attributes
                            if let Some(attr_start) = tag_part.find(' ') {
                                let tag_name = &tag_part[..attr_start];
                                let attributes = &tag_part[attr_start..].trim();
                                if content.is_empty() && trimmed.ends_with("/>") {
                                    Some(format!("  {}: <{}>", tag_name, attributes))
                                } else {
                                    Some(format!("  {}: {} <{}>", tag_name, content, attributes))
                                }
                            } else {
                                Some(format!("  {}: {}", tag_part, content))
                            }
                        } else { None }
                    } else { None }
                } else { None }
            } else { None }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

// Helper to extract attribute value from a tag string
fn extract_attribute_from_tag(tag_str: &str, attr_name: &str) -> Option<String> {
    let attr_prefix_s = format!(" {}='", attr_name);
    let attr_prefix_d = format!(" {}=\"", attr_name);
    if let Some(start) = tag_str.find(&attr_prefix_s) {
        let sub = &tag_str[start + attr_prefix_s.len()..];
        if let Some(end) = sub.find('\'') {
            return Some(sub[..end].to_string());
        }
    } else if let Some(start) = tag_str.find(&attr_prefix_d) {
        let sub = &tag_str[start + attr_prefix_d.len()..];
        if let Some(end) = sub.find('"') {
            return Some(sub[..end].to_string());
        }
    }
    None
}

// Specific formatter for Windows Error Reporting Event ID 1001
fn format_wer_event_data(event_data_xml: &str) -> String {
    use std::collections::HashMap;
    let mut data_map = HashMap::new();

    // Extract Name and content from each <Data> tag
    for line in event_data_xml.lines() {
        if let Some(data_content) = extract_tag_content(line, "Data") {
            if let Some(name) = extract_attribute_from_tag(line, "Name") {
                 data_map.insert(name, data_content.to_string());
            }
        }
    }

    // Build the formatted string based on WER structure
    let mut result = String::new();
    if let (Some(bucket), Some(bucket_type)) = (data_map.get("Bucket"), data_map.get("BucketType")) {
        result.push_str(&format!("Fault bucket {}, type {}\n", bucket, bucket_type));
    }
    if let Some(event_name) = data_map.get("EventName") { result.push_str(&format!("Event Name: {}\n", event_name)); }
    if let Some(response) = data_map.get("Response") { result.push_str(&format!("Response: {}\n", response)); }
    if let Some(cab_id) = data_map.get("CabId") { result.push_str(&format!("Cab Id: {}\n", cab_id)); }

    result.push_str("\nProblem signature:\n");
    for i in 1..=10 {
        let p_key = format!("P{}", i);
        if let Some(val) = data_map.get(&p_key) {
            result.push_str(&format!("P{}: {}\n", i, val));
        }
    }

    if let Some(attached_files) = data_map.get("AttachedFiles") {
        result.push_str("\nAttached files:\n");
        // Split potentially multi-line attached files string
        for file in attached_files.lines() {
            result.push_str(file.trim());
            result.push('\n');
        }
    }

     if let Some(store_path) = data_map.get("StorePath") {
        result.push_str("\nThese files may be available here:\n");
        result.push_str(store_path.trim());
        result.push('\n');
    }

    if let Some(analysis_symbol) = data_map.get("AnalysisSymbol") { result.push_str(&format!("\nAnalysis symbol: {}\n", analysis_symbol)); }
    if let Some(rechecking) = data_map.get("Rechecking") { result.push_str(&format!("Rechecking for solution: {}\n", rechecking)); }
    if let Some(report_id) = data_map.get("ReportId") { result.push_str(&format!("Report Id: {}\n", report_id)); }
    if let Some(report_status) = data_map.get("ReportStatus") { result.push_str(&format!("Report Status: {}\n", report_status)); }
    if let Some(hashed_bucket) = data_map.get("HashedBucket") { result.push_str(&format!("Hashed bucket: {}\n", hashed_bucket)); }
    if let Some(cab_guid) = data_map.get("CabGuid") { result.push_str(&format!("Cab Guid: {}\n", cab_guid)); }

    result.trim_end().to_string()
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
        .constraints([Constraint::Length(25), Constraint::Min(0)])
        .split(frame.size());
    let log_items: Vec<ListItem> = LOG_NAMES.iter().map(|&name| ListItem::new(name)).collect();
    let log_list_block = Block::default().title("Event Viewer (Local)").borders(Borders::ALL).border_style(Style::default().fg(if app_state.focus == PanelFocus::Logs { Color::Cyan } else { Color::White }));
    let log_list = List::new(log_items)
        .block(log_list_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(if app_state.focus == PanelFocus::Logs { Color::Blue } else { Color::DarkGray }))
        .highlight_symbol("> ");
    let mut log_list_state = ListState::default();
    log_list_state.select(Some(app_state.selected_log_index));
    frame.render_stateful_widget(log_list, main_layout[0], &mut log_list_state);

    // Split the right area vertically
    let right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),      // Event table
            Constraint::Length(8), // Preview pane
        ])
        .split(main_layout[1]);

    let events_area = right_layout[0];
    let preview_area = right_layout[1];

    // --- Render Event Table (in events_area) ---
    let event_rows: Vec<Row> = app_state.events.iter().map(|event| {
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
            Cell::from(event.id.clone())
        ])
    }).collect();
    let header_cells = ["Level", "Date and Time", "Source", "Event ID"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::DarkGray)).height(1).bottom_margin(0);
    let widths = [
        Constraint::Length(11),
        Constraint::Length(20),
        Constraint::Percentage(60),
        Constraint::Length(10)
    ];
    let event_table_block = Block::default()
        .title(format!("Events: {}", app_state.selected_log_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app_state.focus == PanelFocus::Events { Color::Cyan } else { Color::White }));
    let event_table = Table::new(event_rows, widths)
        .header(header)
        .block(event_table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .column_spacing(1);
    frame.render_stateful_widget(event_table, events_area, &mut app_state.table_state);

    // --- Render Preview Pane (in preview_area) ---
    let preview_block = Block::default()
        .title("Event Message Preview")
        .borders(Borders::ALL)
        // Highlight border if preview pane is focused
        .border_style(Style::default().fg(if app_state.focus == PanelFocus::Preview { Color::Cyan } else { Color::White }));

    let preview_message = if let Some(selected_index) = app_state.table_state.selected() {
        app_state.events.get(selected_index)
            .map_or("<Message not available>".to_string(), |event| event.message.clone())
    } else {
        "<No event selected>".to_string()
    };

    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        // Apply scroll offset
        .scroll((app_state.preview_scroll, 0));

    frame.render_widget(preview_paragraph, preview_area);

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
                #[cfg(target_os = "windows")]
                if app_state.query_handle.is_none() {
                    app_state.start_or_continue_log_load(true);
                }
                app_state.switch_focus();
            }
            KeyCode::Enter => {
                #[cfg(target_os = "windows")]
                app_state.start_or_continue_log_load(true);
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
            KeyCode::Left | KeyCode::BackTab => app_state.switch_focus(),
            KeyCode::Tab => app_state.switch_focus(),
            _ => {}
        },
        PanelFocus::Preview => match key.code {
            KeyCode::Char('q') => return,
            KeyCode::Up => app_state.preview_scroll_up(),
            KeyCode::Down => app_state.preview_scroll_down(),
            KeyCode::Left => app_state.switch_focus(),
            KeyCode::Tab => app_state.switch_focus(),
            KeyCode::BackTab => app_state.switch_focus(),
            _ => {}
        },
    }
}
