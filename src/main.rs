use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use minidom::Element;
use quick_xml::{Reader, Writer, events::Event as XmlEvent};
use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::block::{Position, Title},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::Cursor,
    io::{self, Stdout, stdout},
    time::Duration,
    vec,
};
use windows::Win32::System::EventLog::{EVT_HANDLE, EvtRender, EvtRenderEventXml};

const EVENT_XML_NS: &str = "http://schemas.microsoft.com/win/2004/08/events/event";
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
        let _ = EvtRender(
            None,
            event_handle,
            EvtRenderEventXml.0,
            0,
            None,
            &mut buffer_used,
            &mut property_count,
        );
        if buffer_used == 0 {
            return None;
        }
        let mut buffer: Vec<u16> = vec![0; buffer_used as usize];
        if EvtRender(
            None,
            event_handle,
            EvtRenderEventXml.0,
            buffer_used,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut buffer_used,
            &mut property_count,
        )
        .is_ok()
        {
            Some(String::from_utf16_lossy(&buffer))
        } else {
            None
        }
    }
}

fn find_attribute_value<'a>(xml: &'a str, attribute_name: &str) -> Option<&'a str> {
    if let Some(start_pos) = xml.find(&format!("{}='", attribute_name)) {
        let sub = &xml[start_pos + attribute_name.len() + 2..];
        sub.find('\'').map(|end_pos| &sub[..end_pos])
    } else if let Some(start_pos) = xml.find(&format!("{}=\"", attribute_name)) {
        let sub = &xml[start_pos + attribute_name.len() + 2..];
        sub.find('"').map(|end_pos| &sub[..end_pos])
    } else {
        None
    }
}

fn get_child_text(parent: &Element, child_name: &str) -> String {
    parent
        .get_child(child_name, EVENT_XML_NS)
        .map_or(String::new(), |el| el.text().to_string())
}

fn get_attr(element: &Element, attr_name: &str) -> Option<String> {
    element.attr(attr_name).map(str::to_string)
}

fn format_wer_event_data_minidom(event_data_element: &Element) -> String {
    let mut data_map: HashMap<String, String> = HashMap::new();
    for data_el in event_data_element
        .children()
        .filter(|c| c.is("Data", EVENT_XML_NS))
    {
        if let Some(name) = data_el.attr("Name") {
            data_map.insert(name.to_string(), data_el.text().to_string());
        }
    }
    let mut result = String::new();
    if let (Some(bucket), Some(bucket_type)) = (data_map.get("Bucket"), data_map.get("BucketType"))
    {
        result.push_str(&format!("Fault bucket {}, type {}\n", bucket, bucket_type));
    }
    if let Some(event_name) = data_map.get("EventName") {
        result.push_str(&format!("Event Name: {}\n", event_name));
    }
    if let Some(response) = data_map.get("Response") {
        result.push_str(&format!("Response: {}\n", response));
    }
    if let Some(cab_id) = data_map.get("CabId") {
        result.push_str(&format!("Cab Id: {}\n", cab_id));
    }
    result.push_str("\nProblem signature:\n");
    for i in 1..=10 {
        let p_key = format!("P{}", i);
        if let Some(val) = data_map.get(&p_key) {
            result.push_str(&format!("P{}: {}\n", i, val));
        }
    }
    if let Some(attached_files) = data_map.get("AttachedFiles") {
        result.push_str("\nAttached files:\n");
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
    if let Some(analysis_symbol) = data_map.get("AnalysisSymbol") {
        result.push_str(&format!("\nAnalysis symbol: {}\n", analysis_symbol));
    }
    if let Some(rechecking) = data_map.get("Rechecking") {
        result.push_str(&format!("Rechecking for solution: {}\n", rechecking));
    }
    if let Some(report_id) = data_map.get("ReportId") {
        result.push_str(&format!("Report Id: {}\n", report_id));
    }
    if let Some(report_status) = data_map.get("ReportStatus") {
        result.push_str(&format!("Report Status: {}\n", report_status));
    }
    if let Some(hashed_bucket) = data_map.get("HashedBucket") {
        result.push_str(&format!("Hashed bucket: {}\n", hashed_bucket));
    }
    if let Some(cab_guid) = data_map.get("CabGuid") {
        result.push_str(&format!("Cab Guid: {}\n", cab_guid));
    }
    result.trim_end().to_string()
}

fn format_generic_event_data_minidom(event_data_element: &Element) -> String {
    event_data_element
        .children()
        .filter(|c| c.is("Data", EVENT_XML_NS))
        .map(|data_el| {
            let name = data_el.attr("Name").unwrap_or("Data");
            let value = data_el.text();
            if value.is_empty() {
                format!("  {}", name)
            } else {
                format!("  {}: {}", name, value)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_event_xml(xml: &str) -> DisplayEvent {
    let root: Result<Element, _> = xml.parse();
    let mut source = "<Parse Error>".to_string();
    let mut id = "0".to_string();
    let mut level = "Unknown".to_string();
    let mut datetime = "".to_string();
    let mut message = "<Parse Error>".to_string();
    if let Ok(root) = root {
        if let Some(system) = root.get_child("System", EVENT_XML_NS) {
            source = system
                .get_child("Provider", EVENT_XML_NS)
                .and_then(|prov| get_attr(prov, "Name"))
                .unwrap_or_else(|| "<Unknown Provider>".to_string());
            if source.starts_with("Microsoft-Windows-") {
                source = source.trim_start_matches("Microsoft-Windows-").to_string();
            }
            id = get_child_text(system, "EventID");
            let level_raw = get_child_text(system, "Level");
            level = match level_raw.as_str() {
                "1" => "Critical".to_string(),
                "2" => "Error".to_string(),
                "3" => "Warning".to_string(),
                "0" | "4" => "Information".to_string(),
                "5" => "Verbose".to_string(),
                _ => format!("Unknown({})", level_raw),
            };
            datetime = system
                .get_child("TimeCreated", EVENT_XML_NS)
                .and_then(|time_el| get_attr(time_el, "SystemTime"))
                .map(|time_str| {
                    chrono::DateTime::parse_from_rfc3339(&time_str)
                        .map(|dt| {
                            dt.with_timezone(&Local)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or(time_str)
                })
                .unwrap_or_default();
        }
        if let Some(event_data) = root.get_child("EventData", EVENT_XML_NS) {
            message = if source == "Windows Error Reporting" && id == "1001" {
                format_wer_event_data_minidom(event_data)
            } else {
                format_generic_event_data_minidom(event_data)
            };
        } else {
            message = "<No EventData found>".to_string();
        }
    }
    DisplayEvent {
        level,
        datetime,
        source,
        id,
        message,
        raw_data: xml.to_string(),
    }
}

#[derive(Clone, Debug)]
struct DisplayEvent {
    level: String,
    datetime: String,
    source: String,
    id: String,
    message: String,
    raw_data: String,
}

#[derive(Debug, Clone)]
struct StatusDialog {
    title: String,
    message: String,
    visible: bool,
    is_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DetailsViewMode {
    Formatted,
    RawXml,
}

#[derive(Debug, Clone)]
struct EventDetailsDialog {
    title: String,
    formatted_content: String,
    raw_xml: String,
    view_mode: DetailsViewMode,
    log_name: String,
    event_id: String,
    event_datetime: String,
    event_source: String,
    visible: bool,
    scroll_position: usize,
    current_visible_height: usize,
}

impl StatusDialog {
    fn new(title: &str, message: &str, is_error: bool) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            visible: true,
            is_error,
        }
    }
    fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl EventDetailsDialog {
    fn new(
        title: &str,
        formatted_content: &str,
        raw_xml: &str,
        log_name: &str,
        event_id: &str,
        event_datetime: &str,
        event_source: &str,
    ) -> Self {
        Self {
            title: title.to_string(),
            formatted_content: formatted_content.to_string(),
            raw_xml: raw_xml.to_string(),
            view_mode: DetailsViewMode::Formatted,
            log_name: log_name.to_string(),
            event_id: event_id.to_string(),
            event_datetime: event_datetime.to_string(),
            event_source: event_source.to_string(),
            visible: true,
            scroll_position: 0,
            current_visible_height: 0,
        }
    }
    fn dismiss(&mut self) {
        self.visible = false;
    }
    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            DetailsViewMode::Formatted => DetailsViewMode::RawXml,
            DetailsViewMode::RawXml => DetailsViewMode::Formatted,
        };
        self.scroll_position = 0;
    }
    fn current_content(&self) -> String {
        match self.view_mode {
            DetailsViewMode::Formatted => self.formatted_content.clone(),
            DetailsViewMode::RawXml => match pretty_print_xml(&self.raw_xml) {
                Ok(pretty) => pretty,
                Err(e) => format!(
                    "<Failed to format Raw XML: {}\n--- Original XML ---\n{}",
                    e, self.raw_xml
                ),
            },
        }
    }
    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }
    fn scroll_down(&mut self, visible_height: usize) {
        let content = self.current_content();
        let content_lines = content.trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        if self.scroll_position < max_scroll {
            self.scroll_position += 1;
        }
    }
    fn page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(10);
    }
    fn page_down(&mut self, visible_height: usize) {
        let content = self.current_content();
        let content_lines = content.trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        self.scroll_position = self.scroll_position.saturating_add(10).min(max_scroll);
    }
    fn go_to_top(&mut self) {
        self.scroll_position = 0;
    }
    fn go_to_bottom(&mut self, visible_height: usize) {
        let content = self.current_content();
        let content_lines = content.trim_end().lines().count();
        self.scroll_position = content_lines.saturating_sub(visible_height.max(1));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EventLevelFilter {
    All,
    Information,
    Warning,
    Error,
}

impl EventLevelFilter {
    fn next(&self) -> Self {
        match self {
            Self::All => Self::Information,
            Self::Information => Self::Warning,
            Self::Warning => Self::Error,
            Self::Error => Self::All,
        }
    }
    fn display_name(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Information => "Info",
            Self::Warning => "Warn",
            Self::Error => "Error/Crit",
        }
    }
    fn to_xpath_query(&self) -> String {
        match self {
            Self::All => "*".to_string(),
            Self::Information => "*[System[Level=0 or Level=4]]".to_string(),
            Self::Warning => "*[System[Level=3]]".to_string(),
            Self::Error => "*[System[Level=1 or Level=2]]".to_string(),
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
    preview_scroll: u16,
    status_dialog: Option<StatusDialog>,
    event_details_dialog: Option<EventDetailsDialog>,
    log_file: Option<std::fs::File>,
    #[cfg(target_os = "windows")]
    query_handle: Option<EVT_HANDLE>,
    is_loading: bool,
    no_more_events: bool,
    sort_descending: bool,
    filter_level: EventLevelFilter,
}

impl AppState {
    fn new() -> Self {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("event_commander.log")
            .ok();
        Self {
            focus: PanelFocus::Logs,
            selected_log_index: 0,
            selected_log_name: String::new(),
            events: Vec::new(),
            table_state: TableState::default(),
            preview_scroll: 0,
            status_dialog: None,
            event_details_dialog: None,
            log_file,
            #[cfg(target_os = "windows")]
            query_handle: None,
            is_loading: false,
            no_more_events: false,
            sort_descending: true,
            filter_level: EventLevelFilter::All,
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
        self.status_dialog = Some(StatusDialog::new(title, message, true));
        self.log(&format!("ERROR - {}: {}", title, message));
    }
    fn show_confirmation(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, false));
        self.log(&format!("INFO - {}: {}", title, message));
    }
    fn show_event_details(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(event) = self.events.get(selected) {
                let title = format!("Event Details: {} ({})", event.source, event.id);
                let mut formatted_content = format!(
                    "Level:       {}\nDateTime:    {}\nSource:      {}\nEvent ID:    {}\n",
                    event.level, event.datetime, event.source, event.id
                );
                formatted_content.push_str("\n--- Message ---\n");
                formatted_content.push_str(&event.message);
                formatted_content.push('\n');
                self.event_details_dialog = Some(EventDetailsDialog::new(
                    &title,
                    &formatted_content,
                    &event.raw_data,
                    &self.selected_log_name,
                    &event.id,
                    &event.datetime,
                    &event.source,
                ));
                self.log(&format!("Showing details for event ID {}", event.id));
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
        if initial_load {
            self.log(&format!(
                "Starting initial load for log: {}",
                self.selected_log_name
            ));
            self.events.clear();
            self.table_state = TableState::default();
            self.no_more_events = false;
            if let Some(handle) = self.query_handle.take() {
                unsafe {
                    let _ = EvtClose(handle);
                }
                self.log("Closed previous query handle.");
            }
            self.selected_log_name = LOG_NAMES
                .get(self.selected_log_index)
                .map(|s| s.to_string())
                .unwrap_or_default();
            if self.selected_log_name.is_empty() {
                self.show_error("Loading Error", "No log name selected.");
                self.is_loading = false;
                return;
            }
            let channel_wide = to_wide_string(&self.selected_log_name);
            let query_str = self.filter_level.to_xpath_query();
            let query_str_wide = to_wide_string(&query_str);
            self.log(&format!("Using query: {}", query_str));
            let flags = if self.sort_descending {
                EvtQueryChannelPath.0 | EvtQueryReverseDirection.0
            } else {
                EvtQueryChannelPath.0
            };
            unsafe {
                match EvtQuery(
                    None,
                    PCWSTR::from_raw(channel_wide.as_ptr()),
                    PCWSTR::from_raw(query_str_wide.as_ptr()),
                    flags,
                ) {
                    Ok(handle) => {
                        self.query_handle = Some(handle);
                        self.log(&format!(
                            "Created new event query handle (Sort Desc: {}).",
                            self.sort_descending
                        ));
                    }
                    Err(e) => {
                        self.show_error(
                            "Query Error",
                            &format!("Failed to query log '{}': {}", self.selected_log_name, e),
                        );
                        self.is_loading = false;
                        return;
                    }
                }
            }
        } else {
            self.log(&format!(
                "Continuing log load... (Sort Desc: {}).",
                self.sort_descending
            ));
        }
        if let Some(query_handle) = self.query_handle {
            let mut new_events_fetched = 0;
            unsafe {
                loop {
                    let mut events_buffer: Vec<EVT_HANDLE> =
                        vec![EVT_HANDLE::default(); EVENT_BATCH_SIZE];
                    let mut fetched: u32 = 0;
                    let events_slice: &mut [isize] =
                        std::mem::transmute(events_buffer.as_mut_slice());
                    let next_result = EvtNext(query_handle, events_slice, 0, 0, &mut fetched);
                    if !next_result.is_ok() {
                        let error = GetLastError().0;
                        if error == ERROR_NO_MORE_ITEMS.0 {
                            self.log("Reached end of event log.");
                            self.no_more_events = true;
                        } else {
                            self.show_error(
                                "Reading Error",
                                &format!(
                                    "Error reading event log '{}': WIN32_ERROR({})",
                                    self.selected_log_name, error
                                ),
                            );
                        }
                        break;
                    }
                    if fetched == 0 {
                        self.log("EvtNext returned 0 events, assuming end.");
                        self.no_more_events = true;
                        break;
                    }
                    for i in 0..(fetched as usize) {
                        let event_handle = events_buffer[i];
                        if let Some(xml) = render_event_xml(event_handle) {
                            self.events.push(parse_event_xml(&xml));
                            new_events_fetched += 1;
                        }
                        let _ = EvtClose(event_handle);
                    }
                    break;
                }
            }
            if new_events_fetched > 0 {
                self.log(&format!(
                    "Fetched {} new events (total {}).",
                    new_events_fetched,
                    self.events.len()
                ));
                if initial_load && !self.events.is_empty() {
                    self.table_state.select(Some(0));
                }
            } else if initial_load {
                self.table_state.select(None);
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
        let new_selection = (current_selection + 1).min(self.events.len().saturating_sub(1));
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
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.select_event(Some(i));
    }
    fn page_down(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let page_size = 10;
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection =
            (current_selection + page_size).min(self.events.len().saturating_sub(1));
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
        let i = self
            .table_state
            .selected()
            .unwrap_or(0)
            .saturating_sub(page_size);
        self.select_event(Some(i));
    }
    fn go_to_top(&mut self) {
        if !self.events.is_empty() {
            self.select_event(Some(0));
        }
    }
    fn go_to_bottom(&mut self) {
        if !self.events.is_empty() {
            let last_index = self.events.len().saturating_sub(1);
            self.select_event(Some(last_index));
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Logs => PanelFocus::Events,
            PanelFocus::Events => PanelFocus::Preview,
            PanelFocus::Preview => PanelFocus::Logs,
        };
    }
    fn preview_scroll_down(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(lines);
    }
    fn preview_scroll_up(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(lines);
    }
    fn preview_go_to_top(&mut self) {
        self.preview_scroll = 0;
    }
    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
    }
    fn select_event(&mut self, index: Option<usize>) {
        self.table_state.select(index);
        self.reset_preview_scroll();
    }
}

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

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
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
    let main_layout =
        Layout::horizontal([Constraint::Max(30), Constraint::Min(0)]).split(frame.size());
    let logs_area = main_layout[0];
    let right_pane_area = main_layout[1];
    let right_layout =
        Layout::vertical([Constraint::Min(0), Constraint::Length(10)]).split(right_pane_area);
    let events_area = right_layout[0];
    let preview_area = right_layout[1];
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
    frame.render_stateful_widget(log_list, logs_area, &mut log_list_state);
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
    let event_table_help_line = Line::from(vec![
        Span::styled("[s]", Style::new().bold().fg(Color::Gray)),
        Span::raw(" sort"),
        Span::styled(" [l]", Style::new().bold().fg(Color::Gray)),
        Span::raw(format!(
            " level ({})",
            app_state.filter_level.display_name()
        )),
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
    let event_table = Table::new(event_rows, widths)
        .header(header)
        .block(event_table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .column_spacing(1);
    frame.render_stateful_widget(event_table, events_area, &mut app_state.table_state);
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
    let preview_message = if let Some(selected_index) = app_state.table_state.selected() {
        app_state
            .events
            .get(selected_index)
            .map_or("<Message not available>".to_string(), |event| {
                event.message.clone()
            })
    } else {
        "<No event selected>".to_string()
    };
    let message_lines = preview_message.lines().count() as u16;
    let available_height = preview_area.height.saturating_sub(2);
    let max_scroll = message_lines.saturating_sub(available_height);
    app_state.preview_scroll = app_state.preview_scroll.min(max_scroll);
    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        .scroll((app_state.preview_scroll, 0));
    frame.render_widget(preview_paragraph, preview_area);
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
            let current_content_string = event_details.current_content();
            let content_lines: Vec<&str> = current_content_string.lines().collect();
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

fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) {
    let current_focus = app_state.focus;
    let mut status_to_show: Option<(String, String, bool)> = None;
    let mut key_handled_by_dialog = false;
    if let Some(dialog) = &mut app_state.event_details_dialog {
        if dialog.visible {
            let visible_height = dialog.current_visible_height;
            match key.code {
                KeyCode::Esc => {
                    dialog.dismiss();
                    key_handled_by_dialog = true;
                }
                KeyCode::Char('v') => {
                    dialog.toggle_view();
                    key_handled_by_dialog = true;
                }
                KeyCode::Char('s') => {
                    let filename = format!(
                        "{}-{}-{}-{}.xml",
                        sanitize_filename(&dialog.log_name),
                        sanitize_filename(&dialog.event_id),
                        dialog.event_datetime.replace(':', "-").replace(' ', "_"),
                        sanitize_filename(&dialog.event_source)
                    );
                    match pretty_print_xml(&dialog.raw_xml) {
                        Ok(pretty_xml) => match fs::write(&filename, &pretty_xml) {
                            Ok(_) => {
                                status_to_show = Some((
                                    "Save Successful".to_string(),
                                    format!("Event saved to:\\n{}", filename),
                                    false,
                                ));
                                dialog.dismiss();
                            }
                            Err(e) => {
                                let err_msg =
                                    format!("Failed to save event to {}: {}", filename, e);
                                app_state.log(&err_msg);
                                status_to_show = Some(("Save Failed".to_string(), err_msg, true));
                            }
                        },
                        Err(err_msg) => {
                            let log_msg = format!("Failed to format XML for saving: {}", err_msg);
                            app_state.log(&log_msg);
                            status_to_show = Some(("Save Failed".to_string(), log_msg, true));
                        }
                    }
                    key_handled_by_dialog = true;
                }
                KeyCode::Up => {
                    dialog.scroll_up();
                    key_handled_by_dialog = true;
                }
                KeyCode::Down => {
                    dialog.scroll_down(visible_height);
                    key_handled_by_dialog = true;
                }
                KeyCode::PageUp => {
                    dialog.page_up();
                    key_handled_by_dialog = true;
                }
                KeyCode::PageDown => {
                    dialog.page_down(visible_height);
                    key_handled_by_dialog = true;
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    dialog.go_to_top();
                    key_handled_by_dialog = true;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    dialog.go_to_bottom(visible_height);
                    key_handled_by_dialog = true;
                }
                _ => {}
            }
        }
    }
    if !key_handled_by_dialog {
        if let Some(dialog) = &mut app_state.status_dialog {
            if dialog.visible {
                if let KeyCode::Enter | KeyCode::Esc = key.code {
                    dialog.dismiss();
                    key_handled_by_dialog = true;
                }
            }
        }
    }
    if let Some((title, msg, is_error)) = status_to_show {
        if is_error {
            app_state.show_error(&title, &msg);
        } else {
            app_state.show_confirmation(&title, &msg);
        }
        key_handled_by_dialog = true;
    }
    if key_handled_by_dialog {
        return;
    }
    match current_focus {
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
            KeyCode::Home | KeyCode::Char('g') => app_state.go_to_top(),
            KeyCode::End | KeyCode::Char('G') => app_state.go_to_bottom(),
            KeyCode::Enter => app_state.show_event_details(),
            KeyCode::Left | KeyCode::BackTab => app_state.switch_focus(),
            KeyCode::Tab => app_state.focus = PanelFocus::Preview,
            KeyCode::Char('s') => {
                app_state.sort_descending = !app_state.sort_descending;
                #[cfg(target_os = "windows")]
                {
                    if let Some(handle) = app_state.query_handle.take() {
                        unsafe {
                            let _ = EvtClose(handle);
                        }
                    }
                    app_state.events.clear();
                    app_state.table_state.select(None);
                    app_state.no_more_events = false;
                    app_state.is_loading = false;
                    app_state.preview_scroll = 0;
                    app_state.start_or_continue_log_load(true);
                }
            }
            KeyCode::Char('l') => {
                app_state.filter_level = app_state.filter_level.next();
                app_state.log(&format!(
                    "Set level filter to: {}",
                    app_state.filter_level.display_name()
                ));
                #[cfg(target_os = "windows")]
                {
                    if let Some(handle) = app_state.query_handle.take() {
                        unsafe {
                            let _ = EvtClose(handle);
                        }
                    }
                    app_state.events.clear();
                    app_state.table_state.select(None);
                    app_state.no_more_events = false;
                    app_state.is_loading = false;
                    app_state.preview_scroll = 0;
                    app_state.start_or_continue_log_load(true);
                }
            }
            _ => {}
        },
        PanelFocus::Preview => match key.code {
            KeyCode::Char('q') => return,
            KeyCode::Up => app_state.preview_scroll_up(1),
            KeyCode::Down => app_state.preview_scroll_down(1),
            KeyCode::PageUp => app_state.preview_scroll_up(10),
            KeyCode::PageDown => app_state.preview_scroll_down(10),
            KeyCode::Home | KeyCode::Char('g') => app_state.preview_go_to_top(),
            KeyCode::Left | KeyCode::BackTab => app_state.switch_focus(),
            KeyCode::Tab => app_state.focus = PanelFocus::Logs,
            _ => {}
        },
    }
}

fn pretty_print_xml(xml_str: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml_str);
    reader.trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => writer
                .write_event(XmlEvent::Start(e))
                .map_err(|e| format!("XML Write Error (Start): {}", e))?,
            Ok(XmlEvent::End(e)) => writer
                .write_event(XmlEvent::End(e))
                .map_err(|e| format!("XML Write Error (End): {}", e))?,
            Ok(XmlEvent::Empty(e)) => writer
                .write_event(XmlEvent::Empty(e))
                .map_err(|e| format!("XML Write Error (Empty): {}", e))?,
            Ok(XmlEvent::Text(e)) => writer
                .write_event(XmlEvent::Text(e))
                .map_err(|e| format!("XML Write Error (Text): {}", e))?,
            Ok(XmlEvent::Comment(e)) => writer
                .write_event(XmlEvent::Comment(e))
                .map_err(|e| format!("XML Write Error (Comment): {}", e))?,
            Ok(XmlEvent::CData(e)) => writer
                .write_event(XmlEvent::CData(e))
                .map_err(|e| format!("XML Write Error (CData): {}", e))?,
            Ok(XmlEvent::Decl(e)) => writer
                .write_event(XmlEvent::Decl(e))
                .map_err(|e| format!("XML Write Error (Decl): {}", e))?,
            Ok(XmlEvent::PI(e)) => writer
                .write_event(XmlEvent::PI(e))
                .map_err(|e| format!("XML Write Error (PI): {}", e))?,
            Ok(XmlEvent::DocType(e)) => writer
                .write_event(XmlEvent::DocType(e))
                .map_err(|e| format!("XML Write Error (DocType): {}", e))?,
            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(format!("XML Read Error: {}", e)),
        }
        buf.clear();
    }
    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 Conversion Error: {}", e))
}
