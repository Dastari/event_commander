// use chrono::Local;
use chrono::{DateTime, Duration, Utc};
use ratatui::text::Text;
use ratatui::widgets::TableState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

#[cfg(target_os = "windows")]
use windows::Win32::System::EventLog::EVT_HANDLE;

/// Represents an event with displayable information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplayEvent {
    pub level: String,
    pub datetime: String,
    pub source: String,
    pub provider_name_original: String,
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
    #[default]
    Formatted,
    RawXml,
}

/// Represents an event level filter for displaying events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum EventLevelFilter {
    #[default]
    All,
    Information,
    Warning,
    Error,
}

/// Represents the time range options for filtering events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TimeFilterOption {
    #[default]
    AnyTime,
    LastHour,
    Last12Hours,
    Last24Hours,
    Last7Days,
    Last30Days,
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
    pub time_filter: TimeFilterOption,
}

/// Represents which field is focused in the filter dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterFieldFocus {
    EventId,
    Level,
    Time,
    Source,
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
    pub preview_content: Option<Text<'static>>,
    pub preview_raw_xml: Option<String>,
    pub preview_view_mode: PreviewViewMode,
    pub log_file: Option<BufWriter<File>>,
    #[cfg(target_os = "windows")]
    pub query_handle: Option<EVT_HANDLE>,
    #[cfg(target_os = "windows")]
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
    pub filter_dialog_time: TimeFilterOption,
    pub available_sources: Option<Vec<String>>,
    pub filter_dialog_source_input: String,
    pub filter_dialog_filtered_sources: Vec<(usize, String)>,
    pub filter_dialog_filtered_source_selection: Option<usize>,
    pub filter_event_id_cursor: usize,
    pub filter_source_cursor: usize,
    pub search_cursor: usize,
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

impl TimeFilterOption {
    /// Cycles to the next time filter option.
    pub fn next(&self) -> Self {
        match self {
            Self::AnyTime => Self::LastHour,
            Self::LastHour => Self::Last12Hours,
            Self::Last12Hours => Self::Last24Hours,
            Self::Last24Hours => Self::Last7Days,
            Self::Last7Days => Self::Last30Days,
            Self::Last30Days => Self::AnyTime,
        }
    }

    /// Cycles to the previous time filter option.
    pub fn previous(&self) -> Self {
        match self {
            Self::AnyTime => Self::Last30Days,
            Self::LastHour => Self::AnyTime,
            Self::Last12Hours => Self::LastHour,
            Self::Last24Hours => Self::Last12Hours,
            Self::Last7Days => Self::Last24Hours,
            Self::Last30Days => Self::Last7Days,
        }
    }

    /// Returns a displayable name for the time filter option.
    pub fn display_name(&self) -> &str {
        match self {
            Self::AnyTime => "Any Time",
            Self::LastHour => "Last Hour",
            Self::Last12Hours => "Last 12 Hours",
            Self::Last24Hours => "Last 24 Hours",
            Self::Last7Days => "Last 7 Days",
            Self::Last30Days => "Last 30 Days",
        }
    }

    /// Calculates the start time for the filter based on the option.
    /// Returns None for AnyTime.
    pub fn get_start_time(&self) -> Option<DateTime<Utc>> {
        let now = Utc::now();
        match self {
            Self::AnyTime => None,
            Self::LastHour => Some(now - Duration::hours(1)),
            Self::Last12Hours => Some(now - Duration::hours(12)),
            Self::Last24Hours => Some(now - Duration::days(1)),
            Self::Last7Days => Some(now - Duration::days(7)),
            Self::Last30Days => Some(now - Duration::days(30)),
        }
    }
}

impl FilterFieldFocus {
    /// Cycles to the next field in the filter dialog.
    pub fn next(&self) -> Self {
        match self {
            Self::EventId => Self::Level,
            Self::Level => Self::Time,
            Self::Time => Self::Source,
            Self::Source => Self::Apply,
            Self::Apply => Self::Clear,
            Self::Clear => Self::EventId,
        }
    }

    /// Cycles to the previous field in the filter dialog.
    pub fn previous(&self) -> Self {
        match self {
            Self::EventId => Self::Clear,
            Self::Level => Self::EventId,
            Self::Time => Self::Level,
            Self::Source => Self::Time,
            Self::Apply => Self::Source,
            Self::Clear => Self::Apply,
        }
    }
}

// NOTE: The `impl AppState { ... }` block has been removed from this file.
// It should reside in `src/app_state.rs`.

// NOTE: The `impl Drop for AppState { ... }` block has been removed from this file.
// It should reside in `src/app_state.rs`.
