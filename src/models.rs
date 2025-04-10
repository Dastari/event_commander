use chrono::Local;
use ratatui::widgets::TableState;
use std::io::Write;

#[cfg(target_os = "windows")]
use windows::Win32::System::EventLog::EVT_HANDLE;

/// Represents an event with displayable information.
#[derive(Clone, Debug)]
pub struct DisplayEvent {
    pub level: String,
    pub datetime: String,
    pub source: String,
    pub id: String,
    pub message: String,
    pub raw_data: String,
}

/// Represents a status dialog with a title, message, and state flags.
#[derive(Debug, Clone)]
pub struct StatusDialog {
    pub title: String,
    pub message: String,
    pub visible: bool,
    pub is_error: bool,
}

/// Represents the view mode for event details: either formatted or raw XML.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailsViewMode {
    Formatted,
    RawXml,
}

/// Trait for components that can be navigated with scroll actions
pub trait Navigable {
    fn scroll_up(&mut self);
    fn scroll_down(&mut self, visible_height: usize);
    fn page_up(&mut self);
    fn page_down(&mut self, visible_height: usize);
    fn go_to_top(&mut self);
    fn go_to_bottom(&mut self, visible_height: usize);
}

/// Contains details for a selected event including formatted content, raw XML, and view state.
#[derive(Debug, Clone)]
pub struct EventDetailsDialog {
    pub title: String,
    pub formatted_content: String,
    pub raw_xml: String,
    pub view_mode: DetailsViewMode,
    pub log_name: String,
    pub event_id: String,
    pub event_datetime: String,
    pub event_source: String,
    pub visible: bool,
    pub scroll_position: usize,
    pub current_visible_height: usize,
}

/// Represents an event level filter for displaying events.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EventLevelFilter {
    #[default]
    All,
    Information,
    Warning,
    Error,
}

/// Represents which panel is currently focused in the TUI.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum PanelFocus {
    Events,
    Preview,
}

/// Represents criteria for filtering events.
#[derive(Debug, Clone, Default)]
pub struct FilterCriteria {
    pub source: Option<String>,
    pub event_id: Option<String>,
    pub level: EventLevelFilter,
}

/// Represents which field is focused in the filter dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterFieldFocus {
    Source,
    EventId,
    Level,
    Apply,
    Clear,
}

/// Represents actions to be taken after a key press is handled.
pub enum PostKeyPressAction {
    None,
    ReloadData,
    ShowConfirmation(String, String),
    OpenFilterDialog,
    Quit,
}

/// Holds the entire state of the application.
pub struct AppState {
    pub focus: PanelFocus,
    pub selected_log_index: usize,
    pub selected_log_name: String,
    pub events: Vec<DisplayEvent>,
    pub table_state: TableState,
    pub preview_scroll: u16,
    pub status_dialog: Option<StatusDialog>,
    pub event_details_dialog: Option<EventDetailsDialog>,
    pub log_file: Option<std::fs::File>,
    #[cfg(target_os = "windows")]
    pub query_handle: Option<EVT_HANDLE>,
    pub is_loading: bool,
    pub no_more_events: bool,
    pub sort_descending: bool,
    pub active_filter: Option<FilterCriteria>,
    pub is_searching: bool,
    pub search_term: String,
    pub last_search_term: Option<String>,
    pub is_filter_dialog_visible: bool,
    pub filter_dialog_focus: FilterFieldFocus,
    pub filter_dialog_source_index: usize,
    pub filter_dialog_event_id: String,
    pub filter_dialog_level: EventLevelFilter,
    pub available_sources: Option<Vec<String>>,
    pub filter_dialog_source_input: String,
    pub filter_dialog_filtered_sources: Vec<(usize, String)>,
    pub filter_dialog_filtered_source_selection: Option<usize>,
    pub help_dialog_visible: bool,
    pub help_scroll_position: usize,
}

// Constants
pub const EVENT_BATCH_SIZE: usize = 1000;
pub const LOG_NAMES: [&str; 5] = [
    "Application",
    "System",
    "Security",
    "Setup",
    "ForwardedEvents",
];

impl StatusDialog {
    /// Creates a new StatusDialog with the given title, message, and error flag.
    pub fn new(title: &str, message: &str, is_error: bool) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            visible: true,
            is_error,
        }
    }
    /// Dismisses the status dialog.
    pub fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl EventDetailsDialog {
    /// Creates a new event details dialog with the provided content and metadata.
    pub fn new(
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
    /// Hides the dialog.
    pub fn dismiss(&mut self) {
        self.visible = false;
    }
    /// Toggles between formatted view and raw XML view.
    pub fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            DetailsViewMode::Formatted => DetailsViewMode::RawXml,
            DetailsViewMode::RawXml => DetailsViewMode::Formatted,
        };
        self.scroll_position = 0;
    }
    /// Returns the content for the current view mode.
    pub fn current_content(&self) -> String {
        match self.view_mode {
            DetailsViewMode::Formatted => self.formatted_content.clone(),
            DetailsViewMode::RawXml => match crate::helpers::pretty_print_xml(&self.raw_xml) {
                Ok(pretty) => pretty,
                Err(e) => format!(
                    "<Failed to format Raw XML: {}\n--- Original XML ---\n{}",
                    e, self.raw_xml
                ),
            },
        }
    }
}

impl Navigable for EventDetailsDialog {
    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    fn scroll_down(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        if self.scroll_position < max_scroll {
            self.scroll_position += 1;
        }
    }

    fn page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(10);
    }

    fn page_down(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        let max_scroll = content_lines.saturating_sub(visible_height.max(1));
        self.scroll_position = self.scroll_position.saturating_add(10).min(max_scroll);
    }

    fn go_to_top(&mut self) {
        self.scroll_position = 0;
    }

    fn go_to_bottom(&mut self, visible_height: usize) {
        let content_lines = self.current_content().trim_end().lines().count();
        self.scroll_position = content_lines.saturating_sub(visible_height.max(1));
    }
}

impl EventLevelFilter {
    /// Cycles to the next event level filter.
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Information,
            Self::Information => Self::Warning,
            Self::Warning => Self::Error,
            Self::Error => Self::All,
        }
    }
    /// Cycles to the previous event level filter.
    pub fn previous(&self) -> Self {
        match self {
            Self::All => Self::Error,
            Self::Information => Self::All,
            Self::Warning => Self::Information,
            Self::Error => Self::Warning,
        }
    }
    /// Returns a display-friendly name for the filter.
    pub fn display_name(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Information => "Info",
            Self::Warning => "Warn",
            Self::Error => "Error/Crit",
        }
    }
}

impl FilterFieldFocus {
    pub fn next(&self) -> Self {
        match self {
            Self::Source => Self::EventId,
            Self::EventId => Self::Level,
            Self::Level => Self::Apply,
            Self::Apply => Self::Clear,
            Self::Clear => Self::Source,
        }
    }
    
    pub fn previous(&self) -> Self {
        match self {
            Self::Source => Self::Clear,
            Self::EventId => Self::Source,
            Self::Level => Self::EventId,
            Self::Apply => Self::Level,
            Self::Clear => Self::Apply,
        }
    }
}

impl AppState {
    /// Creates a new AppState with default configuration and opens the log file.
    pub fn new() -> Self {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("event_commander.log")
            .ok();
        Self {
            focus: PanelFocus::Events,
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
    pub fn log(&mut self, message: &str) {
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
    pub fn show_error(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, true));
        self.log(&format!("ERROR - {}: {}", title, message));
    }
    /// Shows a confirmation dialog with the given title and message.
    pub fn show_confirmation(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, false));
    }
    
    /// Displays event details for the currently selected event.
    pub fn show_event_details(&mut self) {
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
}

#[cfg(target_os = "windows")]
impl Drop for AppState {
    /// Drops AppState and ensures that the Windows Event Log query handle is closed.
    fn drop(&mut self) {
        if let Some(handle) = self.query_handle.take() {
            unsafe {
                let _ = windows::Win32::System::EventLog::EvtClose(handle);
            }
            self.log("ERROR - Failed to close query handle.");
        }
    }
} 