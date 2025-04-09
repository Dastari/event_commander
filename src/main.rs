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
    io::{self, Cursor, Stdout, stdout},
    time::Duration,
    vec,
};

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError},
    Win32::System::EventLog::{
        EVT_HANDLE, EvtClose, EvtNext, EvtNextPublisherId, EvtOpenPublisherEnum, EvtQuery,
        EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender, EvtRenderEventXml,
    },
    core::PCWSTR,
};

const EVENT_XML_NS: &str = "http://schemas.microsoft.com/win/2004/08/events/event";
const EVENT_BATCH_SIZE: usize = 100;
const LOG_NAMES: [&str; 5] = [
    "Application",
    "System",
    "Security",
    "Setup",
    "ForwardedEvents",
];

/// Converts a string slice to a null-terminated wide UTF-16 encoded vector.
#[cfg(target_os = "windows")]
fn to_wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Renders the event XML from an event handle using the Windows Event Log API.
#[cfg(target_os = "windows")]
fn render_event_xml(event_handle: EVT_HANDLE) -> Option<String> {
    unsafe {
        let mut buffer_used = 0;
        let mut property_count = 0;
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

/// Retrieves the text of a child element with the specified name from a parent element.
/// Returns an empty string if the child does not exist.
fn get_child_text(parent: &Element, child_name: &str) -> String {
    parent
        .get_child(child_name, EVENT_XML_NS)
        .map_or(String::new(), |el| el.text().to_string())
}

/// Gets the value of the specified attribute from an element.
fn get_attr(element: &Element, attr_name: &str) -> Option<String> {
    element.attr(attr_name).map(str::to_string)
}

/// Formats Windows Error Reporting event data from a minidom XML element.
#[cfg(target_os = "windows")]
fn format_wer_event_data_minidom(event_data_element: &Element) -> String {
    let mut data_map = HashMap::new();
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

/// Formats generic event data from a minidom XML element.
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

/// Parses an event XML string and returns a DisplayEvent struct with extracted data.
#[cfg(target_os = "windows")]
fn parse_event_xml(xml: &str) -> DisplayEvent {
    let root: Result<Element, _> = xml.parse();
    let mut source = "<Parse Error>".to_string();
    let mut id = "0".to_string();
    let mut level = "Unknown".to_string();
    let mut datetime = String::new();
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

/// Represents an event with displayable information.
#[derive(Clone, Debug)]
struct DisplayEvent {
    level: String,
    datetime: String,
    source: String,
    id: String,
    message: String,
    raw_data: String,
}

/// Represents a status dialog with a title, message, and state flags.
#[derive(Debug, Clone)]
struct StatusDialog {
    title: String,
    message: String,
    visible: bool,
    is_error: bool,
}

/// Represents the view mode for event details: either formatted or raw XML.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DetailsViewMode {
    Formatted,
    RawXml,
}

/// Contains details for a selected event including formatted content, raw XML, and view state.
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
    /// Creates a new StatusDialog with the given title, message, and error flag.
    fn new(title: &str, message: &str, is_error: bool) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            visible: true,
            is_error,
        }
    }
    /// Dismisses the status dialog.
    fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl EventDetailsDialog {
    /// Creates a new EventDetailsDialog with provided details and initializes view state.
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
    /// Dismisses the event details dialog.
    fn dismiss(&mut self) {
        self.visible = false;
    }
    /// Toggles between the formatted and raw XML view modes.
    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            DetailsViewMode::Formatted => DetailsViewMode::RawXml,
            DetailsViewMode::RawXml => DetailsViewMode::Formatted,
        };
        self.scroll_position = 0;
    }
    /// Returns the currently active content based on the view mode.
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
    /// Scrolls the content up by one line.
    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }
    /// Scrolls the content down by one line.
    fn scroll_down(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        if self.scroll_position < max_scroll {
            self.scroll_position += 1;
        }
    }
    /// Scrolls the content up by ten lines (page up).
    fn page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(10);
    }
    /// Scrolls the content down by ten lines (page down).
    fn page_down(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        self.scroll_position = self.scroll_position.saturating_add(10).min(max_scroll);
    }
    /// Scrolls the content to the top.
    fn go_to_top(&mut self) {
        self.scroll_position = 0;
    }
    /// Scrolls the content to the bottom.
    fn go_to_bottom(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        self.scroll_position = content_lines.saturating_sub(visible_height.max(1));
    }
}

/// Represents an event level filter for displaying events.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum EventLevelFilter {
    #[default]
    All,
    Information,
    Warning,
    Error,
}

impl EventLevelFilter {
    /// Cycles to the next event level filter.
    fn next(&self) -> Self {
        match self {
            Self::All => Self::Information,
            Self::Information => Self::Warning,
            Self::Warning => Self::Error,
            Self::Error => Self::All,
        }
    }
    /// Cycles to the previous event level filter.
    fn previous(&self) -> Self {
        match self {
            Self::All => Self::Error,
            Self::Information => Self::All,
            Self::Warning => Self::Information,
            Self::Error => Self::Warning,
        }
    }
    /// Returns a display-friendly name for the filter.
    fn display_name(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Information => "Info",
            Self::Warning => "Warn",
            Self::Error => "Error/Crit",
        }
    }
    /// Returns the corresponding XPath query filter string.
    fn to_xpath_query(&self) -> String {
        match self {
            Self::All => "*".to_string(),
            Self::Information => "*[System[Level=0 or Level=4]]".to_string(),
            Self::Warning => "*[System[Level=3]]".to_string(),
            Self::Error => "*[System[Level=1 or Level=2]]".to_string(),
        }
    }
}

/// Represents which panel is currently focused in the TUI.
#[derive(PartialEq, Debug, Clone, Copy)]
enum PanelFocus {
    Logs,
    Events,
    Preview,
}

/// Represents criteria for filtering events.
#[derive(Debug, Clone, Default)]
struct FilterCriteria {
    source: Option<String>,
    event_id: Option<String>,
    level: EventLevelFilter,
}

/// Represents which field is focused in the filter dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FilterFieldFocus {
    Source,
    EventId,
    Level,
    Apply,
    Clear,
}

/// Represents actions to be taken after a key press is handled.
enum PostKeyPressAction {
    None,
    ReloadData,
    ShowConfirmation(String, String),
    OpenFilterDialog,
    Quit,
}

/// Holds the entire state of the application.
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
    active_filter: Option<FilterCriteria>,
    is_searching: bool,
    search_term: String,
    last_search_term: Option<String>,
    is_filter_dialog_visible: bool,
    filter_dialog_focus: FilterFieldFocus,
    filter_dialog_source_index: usize,
    filter_dialog_event_id: String,
    filter_dialog_level: EventLevelFilter,
    available_sources: Option<Vec<String>>,
    filter_dialog_source_input: String,
    filter_dialog_filtered_sources: Vec<(usize, String)>,
    filter_dialog_filtered_source_selection: Option<usize>,
    help_dialog_visible: bool,
    help_scroll_position: usize,
}

#[cfg(target_os = "windows")]
/// Loads available event log sources using the Windows Event Log API.
fn load_available_sources(app: &mut AppState) -> Option<Vec<String>> {
    let mut sources = Vec::new();
    let publisher_enum_handle = match unsafe { EvtOpenPublisherEnum(None, 0) } {
        Ok(handle) if !handle.is_invalid() => handle,
        Ok(_handle) => return None,
        Err(_e) => {
            app.log(&format!(
                "Error calling EvtOpenPublisherEnum: {} GetLastError: {:?}",
                _e,
                unsafe { GetLastError() }
            ));
            return None;
        }
    };
    let mut buffer: Vec<u16> = Vec::new();
    let mut buffer_size_needed = 0;
    loop {
        let get_size_result =
            unsafe { EvtNextPublisherId(publisher_enum_handle, None, &mut buffer_size_needed) };
        match get_size_result {
            Err(e) if e.code() == ERROR_NO_MORE_ITEMS.into() => break,
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.into() => {
                if buffer_size_needed == 0 {
                    break;
                }
                buffer.resize(buffer_size_needed as usize, 0);
                match unsafe {
                    EvtNextPublisherId(
                        publisher_enum_handle,
                        Some(buffer.as_mut_slice()),
                        &mut buffer_size_needed,
                    )
                } {
                    Ok(_) => {
                        if buffer_size_needed > 0 && (buffer_size_needed as usize) <= buffer.len() {
                            let null_pos = buffer[..buffer_size_needed as usize]
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(buffer_size_needed as usize);
                            if null_pos <= buffer_size_needed as usize {
                                let publisher_id = String::from_utf16_lossy(&buffer[..null_pos]);
                                if !publisher_id.is_empty() {
                                    sources.push(publisher_id);
                                }
                            }
                        }
                    }
                    Err(_e) => break,
                }
            }
            Err(_) => break,
            Ok(_) => break,
        }
    }
    unsafe {
        let _ = EvtClose(publisher_enum_handle);
    }
    if sources.is_empty() {
        None
    } else {
        sources.insert(0, "[Any Source]".to_string());
        sources.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        Some(sources)
    }
}

impl AppState {
    /// Creates a new AppState with default configuration and opens the log file.
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
            active_filter: None,
            is_searching: false,
            search_term: String::new(),
            last_search_term: None,
            is_filter_dialog_visible: false,
            filter_dialog_focus: FilterFieldFocus::Source,
            filter_dialog_source_index: 0,
            filter_dialog_event_id: String::new(),
            filter_dialog_level: EventLevelFilter::All,
            available_sources: None,
            filter_dialog_source_input: String::new(),
            filter_dialog_filtered_sources: Vec::new(),
            filter_dialog_filtered_source_selection: None,
            help_dialog_visible: false,
            help_scroll_position: 0,
        }
    }
    /// Logs a message to the log file if the message indicates an error.
    fn log(&mut self, message: &str) {
        if let Some(file) = &mut self.log_file {
            if message.contains("Error")
                || message.contains("Failed")
                || message.contains("GetLastError")
            {
                use std::io::Write;
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                let log_entry = format!("[{}] {}\n", timestamp, message);
                let _ = file.write_all(log_entry.as_bytes());
                let _ = file.flush();
            }
        }
    }
    /// Shows an error dialog with the given title and message.
    fn show_error(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, true));
        self.log(&format!("ERROR - {}: {}", title, message));
    }
    /// Shows a confirmation dialog with the given title and message.
    fn show_confirmation(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, false));
    }
    /// Displays event details for the currently selected event.
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
            }
        }
    }
    /// Starts or continues loading event logs; initializes the query if it's the initial load.
    #[cfg(target_os = "windows")]
    fn start_or_continue_log_load(&mut self, initial_load: bool) {
        if self.is_loading || (!initial_load && self.no_more_events) {
            return;
        }
        self.is_loading = true;
        if initial_load {
            self.events.clear();
            self.table_state = TableState::default();
            self.no_more_events = false;
            if let Some(handle) = self.query_handle.take() {
                unsafe {
                    let _ = EvtClose(handle);
                }
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
            let query_str = self.build_xpath_from_filter();
            let query_str_wide = to_wide_string(&query_str);
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
                    Ok(handle) => self.query_handle = Some(handle),
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
        }
        if let Some(query_handle) = self.query_handle {
            let mut new_events_fetched = 0;
            unsafe {
                loop {
                    let mut events_buffer: Vec<EVT_HANDLE> =
                        vec![EVT_HANDLE::default(); EVENT_BATCH_SIZE];
                    let mut fetched = 0;
                    let events_slice: &mut [isize] =
                        std::mem::transmute(events_buffer.as_mut_slice());
                    let next_result = EvtNext(query_handle, events_slice, 0, 0, &mut fetched);
                    if !next_result.is_ok() {
                        let error = GetLastError().0;
                        if error == ERROR_NO_MORE_ITEMS.0 {
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
            if new_events_fetched > 0 && initial_load && !self.events.is_empty() {
                self.table_state.select(Some(0));
            }
        }
        self.is_loading = false;
    }
    /// Switches to the next log in the list and clears the active filter.
    fn next_log(&mut self) {
        if self.selected_log_index < LOG_NAMES.len() - 1 {
            self.selected_log_index += 1;
        }
        self.active_filter = None;
    }
    /// Switches to the previous log in the list and clears the active filter.
    fn previous_log(&mut self) {
        self.selected_log_index = self.selected_log_index.saturating_sub(1);
        self.active_filter = None;
    }
    /// Scrolls down one event in the event list; loads more events if near the end.
    fn scroll_down(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection = (current_selection + 1).min(self.events.len().saturating_sub(1));
        self.select_event(Some(new_selection));
        if new_selection >= self.events.len().saturating_sub(20) {
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    /// Scrolls up one event in the event list.
    fn scroll_up(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.select_event(Some(i));
    }
    /// Scrolls down one page in the event list; loads more events if near the end.
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
        if new_selection >= self.events.len().saturating_sub(20) {
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    /// Scrolls up one page in the event list.
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
    /// Selects the top event in the event list.
    fn go_to_top(&mut self) {
        if !self.events.is_empty() {
            self.select_event(Some(0));
        }
    }
    /// Selects the bottom event in the event list and loads more events if necessary.
    fn go_to_bottom(&mut self) {
        if !self.events.is_empty() {
            let last_index = self.events.len().saturating_sub(1);
            self.select_event(Some(last_index));
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }
    /// Cycles the focus among the Logs, Events, and Preview panels.
    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Logs => PanelFocus::Events,
            PanelFocus::Events => PanelFocus::Preview,
            PanelFocus::Preview => PanelFocus::Logs,
        };
    }
    /// Scrolls the preview panel down by a specified number of lines.
    fn preview_scroll_down(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(lines);
    }
    /// Scrolls the preview panel up by a specified number of lines.
    fn preview_scroll_up(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(lines);
    }
    /// Scrolls the preview panel to the top.
    fn preview_go_to_top(&mut self) {
        self.preview_scroll = 0;
    }
    /// Resets the preview scroll position.
    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
    }
    /// Selects an event by index in the event table and resets preview scroll.
    fn select_event(&mut self, index: Option<usize>) {
        self.table_state.select(index);
        self.reset_preview_scroll();
    }
    /// Determines if an event matches the provided search term.
    fn event_matches_search(&self, event: &DisplayEvent, term_lower: &str) -> bool {
        event.level.to_lowercase().contains(term_lower)
            || event.datetime.to_lowercase().contains(term_lower)
            || event.source.to_lowercase().contains(term_lower)
            || event.id.to_lowercase().contains(term_lower)
            || event.message.to_lowercase().contains(term_lower)
    }
    /// Finds the next matching event based on the active search term.
    fn find_next_match(&mut self) -> bool {
        if self.events.is_empty() {
            self.show_confirmation("Search", "No events to search.");
            return false;
        }
        let term = if let Some(t) = self.last_search_term.clone() {
            t
        } else {
            self.show_error("Search Error", "No active search term.");
            return false;
        };
        if term.is_empty() {
            self.show_error("Search Error", "Search term cannot be empty.");
            return false;
        }
        let term_lower = term.to_lowercase();
        let start_index = self.table_state.selected().map_or(0, |i| i + 1);
        for i in start_index..self.events.len() {
            if self.event_matches_search(&self.events[i], &term_lower) {
                self.select_event(Some(i));
                return true;
            }
        }
        for i in 0..start_index {
            if self.event_matches_search(&self.events[i], &term_lower) {
                self.select_event(Some(i));
                return true;
            }
        }
        self.show_confirmation("Search", "No further matches found (searched from top).");
        false
    }
    /// Finds the previous matching event based on the active search term.
    fn find_previous_match(&mut self) -> bool {
        if self.events.is_empty() {
            self.show_confirmation("Search", "No events to search.");
            return false;
        }
        let term = if let Some(t) = self.last_search_term.clone() {
            t
        } else {
            self.show_error("Search Error", "No active search term.");
            return false;
        };
        if term.is_empty() {
            self.show_error("Search Error", "Search term cannot be empty.");
            return false;
        }
        let term_lower = term.to_lowercase();
        let start_index = self.table_state.selected().unwrap_or(0);
        if let Some(effective_start) = start_index.checked_sub(1) {
            for i in (0..=effective_start).rev() {
                if self.event_matches_search(&self.events[i], &term_lower) {
                    self.select_event(Some(i));
                    return true;
                }
            }
        }
        for i in (start_index..self.events.len()).rev() {
            if self.event_matches_search(&self.events[i], &term_lower) {
                self.select_event(Some(i));
                return true;
            }
        }
        self.show_confirmation(
            "Search",
            "No previous matches found (searched from bottom).",
        );
        false
    }
    /// Builds an XPath query string based on the active filter criteria.
    fn build_xpath_from_filter(&self) -> String {
        if let Some(filter) = &self.active_filter {
            let mut conditions = Vec::new();
            if let Some(source) = &filter.source {
                if !source.is_empty() {
                    conditions.push(format!(
                        "System/Provider[@Name='{}']",
                        source.replace('\'', "&apos;").replace('"', "&quot;")
                    ));
                }
            }
            if let Some(id) = &filter.event_id {
                if !id.is_empty() && id.chars().all(char::is_numeric) {
                    conditions.push(format!("System/EventID={}", id));
                }
            }
            let level_condition = match filter.level {
                EventLevelFilter::Information => {
                    Some("(System/Level=0 or System/Level=4)".to_string())
                }
                EventLevelFilter::Warning => Some("System/Level=3".to_string()),
                EventLevelFilter::Error => Some("(System/Level=1 or System/Level=2)".to_string()),
                EventLevelFilter::All => None,
            };
            if let Some(cond) = level_condition {
                conditions.push(cond);
            }
            if conditions.is_empty() {
                "*".to_string()
            } else {
                format!("*[{}]", conditions.join(" and "))
            }
        } else {
            self.filter_level.to_xpath_query()
        }
    }
    /// Updates the filtered source list based on the filter dialog's input.
    fn update_filtered_sources(&mut self) {
        if self.available_sources.is_none() {
            self.filter_dialog_filtered_sources.clear();
            self.filter_dialog_filtered_source_selection = None;
            self.filter_dialog_source_index = 0;
            return;
        }
        let sources = self.available_sources.as_ref().unwrap();
        let input_lower = self.filter_dialog_source_input.to_lowercase();
        self.filter_dialog_filtered_sources = sources
            .iter()
            .enumerate()
            .filter(|(_idx, name)| {
                name.as_str() == "[Any Source]" || name.to_lowercase().contains(&input_lower)
            })
            .map(|(idx, name)| (idx, name.clone()))
            .collect();
        if !self.filter_dialog_filtered_sources.is_empty() {
            let current_original_index = self.filter_dialog_source_index;
            let current_selection_still_valid = self
                .filter_dialog_filtered_sources
                .iter()
                .any(|(idx, _)| *idx == current_original_index);
            if current_selection_still_valid {
                self.filter_dialog_filtered_source_selection = self
                    .filter_dialog_filtered_sources
                    .iter()
                    .position(|(idx, _)| *idx == current_original_index);
            } else {
                self.filter_dialog_source_index = self.filter_dialog_filtered_sources[0].0;
                self.filter_dialog_filtered_source_selection = Some(0);
            }
        } else {
            self.filter_dialog_filtered_source_selection = None;
            self.filter_dialog_source_index = 0;
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for AppState {
    /// Drops AppState and ensures that the Windows Event Log query handle is closed.
    fn drop(&mut self) {
        if let Some(handle) = self.query_handle.take() {
            unsafe {
                let _ = EvtClose(handle);
            }
            self.log("ERROR - Failed to close query handle.");
        }
    }
}

/// Sanitizes a filename by retaining only alphanumeric characters, dashes, underscores, and dots.
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

/// Initializes the terminal in raw mode and enters the alternate screen.
fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

/// Restores the terminal to its previous state and leaves the alternate screen.
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// Computes a centered fixed-size rectangle within a given rectangle.
fn centered_fixed_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}

/// Renders the entire UI, including the log list, event table, preview panel, dialogs, and help screen.
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
            app_state.filter_level.display_name()
        )),
        Span::styled("[f]", Style::new().bold().fg(Color::Gray)),
        Span::raw(format!(
            " filter ({}) ",
            if app_state.active_filter.is_some() {
                "Active"
            } else {
                "Inactive"
            }
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
    let event_table = Table::new(event_rows, widths)
        .header(header)
        .block(event_table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .column_spacing(1);
    frame.render_stateful_widget(event_table, events_area, &mut app_state.table_state);
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
    let preview_message = if let Some(selected_index) = app_state.table_state.selected() {
        app_state
            .events
            .get(selected_index)
            .map_or("<Message not available>".to_string(), |e| e.message.clone())
    } else {
        "<No event selected>".to_string()
    };
    let message_lines = preview_message.lines().count() as u16;
    let available_height = preview_area.height.saturating_sub(2);
    app_state.preview_scroll = app_state
        .preview_scroll
        .min(message_lines.saturating_sub(available_height));
    let preview_paragraph = Paragraph::new(preview_message)
        .block(preview_block)
        .wrap(Wrap { trim: true })
        .scroll((app_state.preview_scroll, 0));
    frame.render_widget(preview_paragraph, preview_area);
    let version_string = format!("v{}", VERSION);
    let version_width = version_string.len() as u16;
    if preview_area.width > version_width + 2 && preview_area.height > 1 {
        let version_x = preview_area.right() - version_width - 1;
        let version_y = preview_area.bottom() - 1;
        let version_rect = Rect::new(version_x, version_y, version_width, 1);
        let version_paragraph =
            Paragraph::new(version_string).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(version_paragraph, version_rect);
    }
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
    if app_state.is_filter_dialog_visible {
        let dialog_width = 60;
        let list_visible = app_state.filter_dialog_focus == FilterFieldFocus::Source
            && !app_state.filter_dialog_filtered_sources.is_empty();
        let list_height = if list_visible {
            5.min(app_state.filter_dialog_filtered_sources.len() as u16)
                .max(1)
        } else {
            1
        };
        let required_inner_height = 7 + list_height;
        let dialog_height = required_inner_height + 2 + 2;
        let dialog_area = centered_fixed_rect(
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
        let source_style = if app_state.filter_dialog_focus == FilterFieldFocus::Source {
            focused_style
        } else {
            unfocused_style
        };
        let source_input_display = if app_state.filter_dialog_focus == FilterFieldFocus::Source {
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
        if app_state.filter_dialog_focus == FilterFieldFocus::Source
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
        let event_id_input_style = if app_state.filter_dialog_focus == FilterFieldFocus::EventId {
            focused_style
        } else {
            unfocused_style
        };
        let event_id_text = if app_state.filter_dialog_focus == FilterFieldFocus::EventId {
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
                if app_state.filter_dialog_focus == FilterFieldFocus::Level {
                    focused_style.add_modifier(Modifier::BOLD)
                } else {
                    unfocused_style
                },
            ),
            Span::styled(" >", Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(level_text), chunks[3]);
        let apply_style = if app_state.filter_dialog_focus == FilterFieldFocus::Apply {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let clear_style = if app_state.filter_dialog_focus == FilterFieldFocus::Clear {
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
    if app_state.help_dialog_visible {
        let help_width = 80.min(frame.size().width.saturating_sub(4));
        let help_height = 30.min(frame.size().height.saturating_sub(4));
        let help_area = centered_fixed_rect(help_width, help_height, frame.size());
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
            Line::from(""),
            Line::from(Span::styled(
                "Log List Panel (Left):",
                Style::default().underlined(),
            )),
            Line::from(vec![
                Span::styled("  ↑/↓     ", Style::default().bold()),
                Span::raw("Navigate log types"),
            ]),
            Line::from(vec![
                Span::styled("  Enter   ", Style::default().bold()),
                Span::raw("Select log and move focus to Events"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Event Table Panel (Center):",
                Style::default().underlined(),
            )),
            Line::from(vec![
                Span::styled("  ↑/↓     ", Style::default().bold()),
                Span::raw("Scroll events line by line"),
            ]),
            Line::from(vec![
                Span::styled("  PgUp    ", Style::default().bold()),
                Span::raw("Scroll events up one page"),
            ]),
            Line::from(vec![
                Span::styled("  PgDn    ", Style::default().bold()),
                Span::raw("Scroll events down one page"),
            ]),
            Line::from(vec![
                Span::styled("  Home/g  ", Style::default().bold()),
                Span::raw("Go to the first event"),
            ]),
            Line::from(vec![
                Span::styled("  End/G   ", Style::default().bold()),
                Span::raw("Go to the last event (loads more if needed)"),
            ]),
            Line::from(vec![
                Span::styled("  Enter   ", Style::default().bold()),
                Span::raw("Show detailed view for selected event"),
            ]),
            Line::from(vec![
                Span::styled("  s       ", Style::default().bold()),
                Span::raw("Toggle sort order (Ascending/Descending)"),
            ]),
            Line::from(vec![
                Span::styled("  l       ", Style::default().bold()),
                Span::raw("Cycle through quick level filters (All -> Info -> Warn -> Err/Crit)"),
            ]),
            Line::from(vec![
                Span::styled("  f       ", Style::default().bold()),
                Span::raw("Open detailed filter dialog"),
            ]),
            Line::from(vec![
                Span::styled("  /       ", Style::default().bold()),
                Span::raw("Start search input"),
            ]),
            Line::from(vec![
                Span::styled("  n       ", Style::default().bold()),
                Span::raw("Find next search match (after search)"),
            ]),
            Line::from(vec![
                Span::styled("  p/N     ", Style::default().bold()),
                Span::raw("Find previous search match (after search)"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Event Preview Panel (Bottom Right):",
                Style::default().underlined(),
            )),
            Line::from(vec![
                Span::styled("  ↑/↓     ", Style::default().bold()),
                Span::raw("Scroll preview line by line"),
            ]),
            Line::from(vec![
                Span::styled("  PgUp    ", Style::default().bold()),
                Span::raw("Scroll preview up one page"),
            ]),
            Line::from(vec![
                Span::styled("  PgDn    ", Style::default().bold()),
                Span::raw("Scroll preview down one page"),
            ]),
            Line::from(vec![
                Span::styled("  Home/g  ", Style::default().bold()),
                Span::raw("Scroll preview to top"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Event Details Dialog:",
                Style::default().underlined(),
            )),
            Line::from(vec![
                Span::styled("  Esc     ", Style::default().bold()),
                Span::raw("Dismiss dialog"),
            ]),
            Line::from(vec![
                Span::styled("  v       ", Style::default().bold()),
                Span::raw("Toggle view (Formatted / Raw XML)"),
            ]),
            Line::from(vec![
                Span::styled("  s       ", Style::default().bold()),
                Span::raw("Save current event XML to disk"),
            ]),
            Line::from(vec![
                Span::styled("  ↑/↓     ", Style::default().bold()),
                Span::raw("Scroll content line by line"),
            ]),
            Line::from(vec![
                Span::styled("  PgUp    ", Style::default().bold()),
                Span::raw("Scroll content up one page"),
            ]),
            Line::from(vec![
                Span::styled("  PgDn    ", Style::default().bold()),
                Span::raw("Scroll content down one page"),
            ]),
            Line::from(vec![
                Span::styled("  Home/g  ", Style::default().bold()),
                Span::raw("Go to top of content"),
            ]),
            Line::from(vec![
                Span::styled("  End/G   ", Style::default().bold()),
                Span::raw("Go to bottom of content"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Filter Dialog:",
                Style::default().underlined(),
            )),
            Line::from(vec![
                Span::styled("  Esc     ", Style::default().bold()),
                Span::raw("Cancel filtering and close dialog"),
            ]),
            Line::from(vec![
                Span::styled("  Tab     ", Style::default().bold()),
                Span::raw("Cycle focus through fields/buttons"),
            ]),
            Line::from(vec![
                Span::styled("  S-Tab   ", Style::default().bold()),
                Span::raw("Cycle focus backward"),
            ]),
            Line::from(vec![
                Span::styled("  Enter   ", Style::default().bold()),
                Span::raw("Confirm source selection / Apply / Clear"),
            ]),
            Line::from(vec![
                Span::styled("  ↑/↓     ", Style::default().bold()),
                Span::raw("Navigate source list (when Source focused)"),
            ]),
            Line::from(vec![
                Span::styled("  ←/→     ", Style::default().bold()),
                Span::raw("Change Level selection (when Level focused)"),
            ]),
            Line::from(vec![
                Span::styled("  Chars   ", Style::default().bold()),
                Span::raw("Type in Source/EventID fields"),
            ]),
            Line::from(vec![
                Span::styled("  Bksp    ", Style::default().bold()),
                Span::raw("Delete character in Source/EventID fields"),
            ]),
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

/// Processes a key press event, updates the application state, and returns a PostKeyPressAction.
fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    if app_state.help_dialog_visible {
        match key.code {
            KeyCode::Esc => {
                app_state.help_dialog_visible = false;
                app_state.help_scroll_position = 0;
            }
            KeyCode::Up => {
                app_state.help_scroll_position = app_state.help_scroll_position.saturating_sub(1);
            }
            KeyCode::Down => {
                app_state.help_scroll_position = app_state.help_scroll_position.saturating_add(1);
            }
            KeyCode::PageUp => {
                app_state.help_scroll_position = app_state.help_scroll_position.saturating_sub(10);
            }
            KeyCode::PageDown => {
                app_state.help_scroll_position = app_state.help_scroll_position.saturating_add(10);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                app_state.help_scroll_position = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                app_state.help_scroll_position = usize::MAX;
            }
            _ => {}
        }
        return PostKeyPressAction::None;
    }
    match key.code {
        KeyCode::Char('q') => return PostKeyPressAction::Quit,
        KeyCode::F(1) => {
            app_state.help_dialog_visible = true;
            return PostKeyPressAction::None;
        }
        _ => {}
    }
    if let Some(dialog) = &mut app_state.status_dialog {
        if dialog.visible {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    dialog.dismiss();
                    app_state.log("ERROR - Status dialog dismissed.");
                }
                _ => {
                    app_state.log(&format!("Ignored key {:?} in status dialog.", key.code));
                }
            }
            return PostKeyPressAction::None;
        }
    }
    if app_state.is_searching {
        match key.code {
            KeyCode::Esc => {
                app_state.is_searching = false;
                app_state.search_term.clear();
                return PostKeyPressAction::None;
            }
            KeyCode::Enter => {
                if !app_state.search_term.is_empty() {
                    app_state.is_searching = false;
                    app_state.last_search_term = Some(app_state.search_term.clone());
                    let _result = app_state.find_next_match();
                    app_state.search_term.clear();
                    return PostKeyPressAction::None;
                } else {
                    app_state.is_searching = false;
                    app_state.search_term.clear();
                    return PostKeyPressAction::None;
                }
            }
            KeyCode::Char(c) => {
                app_state.search_term.push(c);
                return PostKeyPressAction::None;
            }
            KeyCode::Backspace => {
                app_state.search_term.pop();
                return PostKeyPressAction::None;
            }
            _ => {
                return PostKeyPressAction::None;
            }
        }
    }
    let mut status_action = PostKeyPressAction::None;
    let mut key_handled = false;
    if let Some(dialog) = &mut app_state.event_details_dialog {
        if dialog.visible {
            match key.code {
                KeyCode::Esc => {
                    dialog.dismiss();
                    key_handled = true;
                }
                KeyCode::Char('v') => {
                    dialog.toggle_view();
                    key_handled = true;
                }
                KeyCode::Char('s') => {
                    key_handled = true;
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
                                status_action = PostKeyPressAction::ShowConfirmation(
                                    "Save Successful".to_string(),
                                    format!("Event saved to:\n{}", filename),
                                );
                                dialog.dismiss();
                            }
                            Err(e) => {
                                let err_msg =
                                    format!("Failed to save event to {}: {}", filename, e);
                                status_action = PostKeyPressAction::ShowConfirmation(
                                    "Save Failed".to_string(),
                                    err_msg,
                                );
                            }
                        },
                        Err(err_msg) => {
                            let log_msg = format!("Failed to format XML for saving: {}", err_msg);
                            status_action = PostKeyPressAction::ShowConfirmation(
                                "Save Failed".to_string(),
                                log_msg,
                            );
                        }
                    }
                }
                KeyCode::Up => {
                    dialog.scroll_up();
                    key_handled = true;
                }
                KeyCode::Down => {
                    dialog.scroll_down(dialog.current_visible_height);
                    key_handled = true;
                }
                KeyCode::PageUp => {
                    dialog.page_up();
                    key_handled = true;
                }
                KeyCode::PageDown => {
                    dialog.page_down(dialog.current_visible_height);
                    key_handled = true;
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    dialog.go_to_top();
                    key_handled = true;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    dialog.go_to_bottom(dialog.current_visible_height);
                    key_handled = true;
                }
                _ => {}
            }
        }
    }
    if key_handled {
        return status_action;
    }
    if app_state.is_filter_dialog_visible {
        app_state.log(&format!(
            "Filter Dialog Key: {:?}, Focus: {:?}",
            key.code, app_state.filter_dialog_focus
        ));
        match key.code {
            KeyCode::Esc => {
                app_state.is_filter_dialog_visible = false;
                return PostKeyPressAction::None;
            }
            KeyCode::Tab => {
                app_state.filter_dialog_focus = match app_state.filter_dialog_focus {
                    FilterFieldFocus::Source => FilterFieldFocus::EventId,
                    FilterFieldFocus::EventId => FilterFieldFocus::Level,
                    FilterFieldFocus::Level => FilterFieldFocus::Apply,
                    FilterFieldFocus::Apply => FilterFieldFocus::Clear,
                    FilterFieldFocus::Clear => FilterFieldFocus::Source,
                };
            }
            KeyCode::BackTab => {
                app_state.filter_dialog_focus = match app_state.filter_dialog_focus {
                    FilterFieldFocus::Source => FilterFieldFocus::Clear,
                    FilterFieldFocus::EventId => FilterFieldFocus::Source,
                    FilterFieldFocus::Level => FilterFieldFocus::EventId,
                    FilterFieldFocus::Apply => FilterFieldFocus::Level,
                    FilterFieldFocus::Clear => FilterFieldFocus::Apply,
                };
            }
            KeyCode::Enter => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {
                    if let Some(selected_pos) = app_state.filter_dialog_filtered_source_selection {
                        if let Some((_, name)) =
                            app_state.filter_dialog_filtered_sources.get(selected_pos)
                        {
                            app_state.filter_dialog_source_input = name.clone();
                            if let Some(original_sources) = &app_state.available_sources {
                                if let Some(idx) = original_sources.iter().position(|s| s == name) {
                                    app_state.filter_dialog_source_index = idx;
                                }
                            }
                            app_state.update_filtered_sources();
                        }
                    }
                    app_state.filter_dialog_focus = FilterFieldFocus::EventId;
                }
                FilterFieldFocus::EventId => {
                    app_state.filter_dialog_focus = FilterFieldFocus::Level;
                }
                FilterFieldFocus::Level => {
                    app_state.filter_dialog_focus = FilterFieldFocus::Apply;
                }
                FilterFieldFocus::Apply => {
                    let source_input_trimmed = app_state.filter_dialog_source_input.trim();
                    let selected_source = if source_input_trimmed.is_empty() {
                        None
                    } else {
                        Some(source_input_trimmed.to_string())
                    };
                    let criteria = FilterCriteria {
                        source: selected_source,
                        event_id: if app_state.filter_dialog_event_id.trim().is_empty() {
                            None
                        } else {
                            Some(app_state.filter_dialog_event_id.trim().to_string())
                        },
                        level: app_state.filter_dialog_level,
                    };
                    if criteria.source.is_none()
                        && criteria.event_id.is_none()
                        && criteria.level == EventLevelFilter::All
                    {
                        app_state.active_filter = None;
                    } else {
                        app_state.active_filter = Some(criteria);
                    }
                    app_state.is_filter_dialog_visible = false;
                    return PostKeyPressAction::ReloadData;
                }
                FilterFieldFocus::Clear => {
                    app_state.active_filter = None;
                    app_state.is_filter_dialog_visible = false;
                    return PostKeyPressAction::ReloadData;
                }
            },
            KeyCode::Char(c) => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {
                    app_state.filter_dialog_source_input.push(c);
                    app_state.update_filtered_sources();
                    if !app_state.filter_dialog_filtered_sources.is_empty() {
                        if app_state.filter_dialog_filtered_source_selection.is_none() {
                            app_state.filter_dialog_filtered_source_selection = Some(0);
                            app_state.filter_dialog_source_index =
                                app_state.filter_dialog_filtered_sources[0].0;
                        }
                    }
                }
                FilterFieldFocus::EventId => {
                    if c.is_ascii_digit() {
                        app_state.filter_dialog_event_id.push(c);
                    }
                }
                _ => {}
            },
            KeyCode::Backspace => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {
                    app_state.filter_dialog_source_input.pop();
                    app_state.update_filtered_sources();
                    if !app_state.filter_dialog_filtered_sources.is_empty() {
                        if app_state.filter_dialog_filtered_source_selection.is_none() {
                            app_state.filter_dialog_filtered_source_selection = Some(0);
                            app_state.filter_dialog_source_index =
                                app_state.filter_dialog_filtered_sources[0].0;
                        }
                    }
                }
                FilterFieldFocus::EventId => {
                    app_state.filter_dialog_event_id.pop();
                }
                _ => {}
            },
            KeyCode::Left => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {}
                FilterFieldFocus::Level => {
                    app_state.filter_dialog_level = app_state.filter_dialog_level.previous();
                }
                _ => {}
            },
            KeyCode::Right => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {}
                FilterFieldFocus::Level => {
                    app_state.filter_dialog_level = app_state.filter_dialog_level.next();
                }
                _ => {}
            },
            KeyCode::Up => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {
                    if !app_state.filter_dialog_filtered_sources.is_empty() {
                        let current_pos = app_state
                            .filter_dialog_filtered_source_selection
                            .unwrap_or(0);
                        let new_pos = if current_pos > 0 {
                            current_pos - 1
                        } else {
                            app_state.filter_dialog_filtered_sources.len() - 1
                        };
                        app_state.filter_dialog_filtered_source_selection = Some(new_pos);
                        if let Some(&(idx, _)) =
                            app_state.filter_dialog_filtered_sources.get(new_pos)
                        {
                            app_state.filter_dialog_source_index = idx;
                        }
                    }
                }
                _ => {}
            },
            KeyCode::Down => match app_state.filter_dialog_focus {
                FilterFieldFocus::Source => {
                    if !app_state.filter_dialog_filtered_sources.is_empty() {
                        let current_pos = app_state
                            .filter_dialog_filtered_source_selection
                            .unwrap_or(0);
                        let new_pos =
                            if current_pos + 1 < app_state.filter_dialog_filtered_sources.len() {
                                current_pos + 1
                            } else {
                                0
                            };
                        app_state.filter_dialog_filtered_source_selection = Some(new_pos);
                        if let Some(&(idx, _)) =
                            app_state.filter_dialog_filtered_sources.get(new_pos)
                        {
                            app_state.filter_dialog_source_index = idx;
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
        return PostKeyPressAction::None;
    }
    if let Some(dialog) = &mut app_state.event_details_dialog {
        if dialog.visible {
            match key.code {
                KeyCode::Esc => {
                    dialog.dismiss();
                    return status_action;
                }
                KeyCode::Char('v') => {
                    dialog.toggle_view();
                    return status_action;
                }
                KeyCode::Char('s') => {
                    let filename = format!(
                        "{}-{}-{}-{}.xml",
                        sanitize_filename(&dialog.log_name),
                        sanitize_filename(&dialog.event_id),
                        dialog.event_datetime.replace(':', "-").replace(' ', "_"),
                        sanitize_filename(&dialog.event_source)
                    );
                    if let Ok(pretty_xml) = pretty_print_xml(&dialog.raw_xml) {
                        if fs::write(&filename, &pretty_xml).is_ok() {
                            status_action = PostKeyPressAction::ShowConfirmation(
                                "Save Successful".to_string(),
                                format!("Event saved to:\n{}", filename),
                            );
                            dialog.dismiss();
                        } else {
                            status_action = PostKeyPressAction::ShowConfirmation(
                                "Save Failed".to_string(),
                                format!("Failed to save event to {}.", filename),
                            );
                        }
                    } else {
                        status_action = PostKeyPressAction::ShowConfirmation(
                            "Save Failed".to_string(),
                            "Failed to format XML for saving.".to_string(),
                        );
                    }
                    return status_action;
                }
                KeyCode::Up => {
                    dialog.scroll_up();
                    return status_action;
                }
                KeyCode::Down => {
                    dialog.scroll_down(dialog.current_visible_height);
                    return status_action;
                }
                KeyCode::PageUp => {
                    dialog.page_up();
                    return status_action;
                }
                KeyCode::PageDown => {
                    dialog.page_down(dialog.current_visible_height);
                    return status_action;
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    dialog.go_to_top();
                    return status_action;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    dialog.go_to_bottom(dialog.current_visible_height);
                    return status_action;
                }
                _ => {}
            }
        }
    }
    match app_state.focus {
        PanelFocus::Logs => match key.code {
            KeyCode::Up => {
                app_state.previous_log();
                return PostKeyPressAction::ReloadData;
            }
            KeyCode::Down => {
                app_state.next_log();
                return PostKeyPressAction::ReloadData;
            }
            KeyCode::Right | KeyCode::Tab => {
                app_state.switch_focus();
            }
            KeyCode::Enter => {
                app_state.switch_focus();
            }
            _ => {}
        },
        PanelFocus::Events => match key.code {
            KeyCode::Up => {
                app_state.scroll_up();
            }
            KeyCode::Down => {
                app_state.scroll_down();
            }
            KeyCode::PageUp => {
                app_state.page_up();
            }
            KeyCode::PageDown => {
                app_state.page_down();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                app_state.go_to_top();
            }
            KeyCode::End | KeyCode::Char('G') => {
                app_state.go_to_bottom();
            }
            KeyCode::Enter => {
                app_state.show_event_details();
            }
            KeyCode::Left | KeyCode::BackTab => {
                app_state.switch_focus();
            }
            KeyCode::Tab => {
                app_state.focus = PanelFocus::Preview;
            }
            KeyCode::Char('s') => {
                app_state.sort_descending = !app_state.sort_descending;
                return PostKeyPressAction::ReloadData;
            }
            KeyCode::Char('l') => {
                app_state.filter_level = app_state.filter_level.next();
                return PostKeyPressAction::ReloadData;
            }
            KeyCode::Char('f') => {
                return PostKeyPressAction::OpenFilterDialog;
            }
            KeyCode::Char('/') => {
                app_state.is_searching = true;
                app_state.search_term.clear();
            }
            KeyCode::Char('n') => {
                if app_state.last_search_term.is_some() {
                    let _ = app_state.find_next_match();
                }
            }
            KeyCode::Char('p') | KeyCode::Char('N') => {
                if app_state.last_search_term.is_some() {
                    let _ = app_state.find_previous_match();
                }
            }
            _ => {}
        },
        PanelFocus::Preview => match key.code {
            KeyCode::Up => {
                app_state.preview_scroll_up(1);
            }
            KeyCode::Down => {
                app_state.preview_scroll_down(1);
            }
            KeyCode::PageUp => {
                app_state.preview_scroll_up(10);
            }
            KeyCode::PageDown => {
                app_state.preview_scroll_down(10);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                app_state.preview_go_to_top();
            }
            KeyCode::Left | KeyCode::BackTab => {
                app_state.switch_focus();
            }
            KeyCode::Tab => {
                app_state.focus = PanelFocus::Logs;
            }
            KeyCode::F(1) => {
                app_state.help_dialog_visible = true;
                return PostKeyPressAction::None;
            }
            _ => {}
        },
    }
    PostKeyPressAction::None
}

/// Formats an XML string with indentation and returns the formatted XML or an error message.
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

/// Application entry point; initializes the terminal and application state, and processes events.
fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = init_terminal()?;
    let mut app_state = AppState::new();
    #[cfg(target_os = "windows")]
    app_state.start_or_continue_log_load(true);
    loop {
        terminal.draw(|frame| ui(frame, &mut app_state))?;
        let mut post_action = PostKeyPressAction::None;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    post_action = handle_key_press(key, &mut app_state);
                }
            }
        }
        match post_action {
            PostKeyPressAction::ReloadData => {
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
            PostKeyPressAction::ShowConfirmation(title, msg) => {
                app_state.show_confirmation(&title, &msg);
            }
            PostKeyPressAction::OpenFilterDialog => {
                if app_state.available_sources.is_none() {
                    #[cfg(target_os = "windows")]
                    {
                        app_state.available_sources = load_available_sources(&mut app_state);
                    }
                }
                app_state.filter_dialog_source_index = 0;
                if let Some(active) = &app_state.active_filter {
                    if let Some(ref source) = active.source {
                        app_state.filter_dialog_source_input = source.clone();
                        if let Some(ref sources) = app_state.available_sources {
                            if let Some(idx) = sources.iter().position(|s| s == source) {
                                app_state.filter_dialog_source_index = idx;
                            }
                        }
                    } else {
                        app_state.filter_dialog_source_input.clear();
                    }
                    app_state.filter_dialog_event_id = active.event_id.clone().unwrap_or_default();
                    app_state.filter_dialog_level = active.level;
                } else {
                    app_state.filter_dialog_source_input.clear();
                    app_state.filter_dialog_event_id.clear();
                    app_state.filter_dialog_level = EventLevelFilter::All;
                }
                app_state.update_filtered_sources();
                app_state.filter_dialog_focus = FilterFieldFocus::Source;
                app_state.is_filter_dialog_visible = true;
            }
            PostKeyPressAction::Quit => break,
            PostKeyPressAction::None => {}
        }
    }
    restore_terminal()?;
    Ok(())
}
