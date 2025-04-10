// use chrono::Local;
use ratatui::widgets::TableState;
use std::io::{BufWriter};
use std::fs::File;
use std::collections::HashMap;

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
    pub formatted_message: Option<String>,
}

/// Represents a status dialog with a title, message, and state flags.
#[derive(Debug, Clone)]
pub struct StatusDialog {
    pub title: String,
    pub message: String,
    pub visible: bool,
    pub is_error: bool,
}

/// Represents the view mode for the preview panel when focused.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PreviewViewMode {
    #[default] // Default to formatted view
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
    pub preview_scroll: usize,
    pub status_dialog: Option<StatusDialog>,
    pub preview_event_id: Option<String>,
    pub preview_formatted_content: Option<String>,
    pub preview_raw_xml: Option<String>,
    pub preview_friendly_message: Option<String>,
    pub preview_view_mode: PreviewViewMode,
    // Use BufWriter<File> for buffered logging
    pub log_file: Option<BufWriter<File>>,
    #[cfg(target_os = "windows")]
    pub query_handle: Option<EVT_HANDLE>,
    #[cfg(target_os = "windows")] // Cache for publisher metadata handles
    pub publisher_metadata_cache: HashMap<String, EVT_HANDLE>,
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

impl EventLevelFilter {
    /// Cycles to the next event level filter.
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Information,
            Self::Information => Self::Warning,
            Self::Warning => Self::Error,
            Self::Error => Self::All, // Wrap around
        }
    }
    /// Cycles to the previous event level filter.
    pub fn previous(&self) -> Self {
        match self {
            Self::All => Self::Error, // Wrap around
            Self::Information => Self::All,
            Self::Warning => Self::Information,
            Self::Error => Self::Warning,
        }
    }
    /// Returns a displayable name for the filter level.
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
            Self::Clear => Self::Source, // Wrap around
        }
    }
    pub fn previous(&self) -> Self {
        match self {
            Self::Source => Self::Clear, // Wrap around
            Self::EventId => Self::Source,
            Self::Level => Self::EventId,
            Self::Apply => Self::Level,
            Self::Clear => Self::Apply,
        }
    }
}

// NOTE: The `impl AppState { ... }` block has been removed from this file.
// It should reside in `src/app_state.rs`.

// NOTE: The `impl Drop for AppState { ... }` block has been removed from this file.
// It should reside in `src/app_state.rs`. 