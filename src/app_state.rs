use crate::models::{
    AppState, DisplayEvent, EventLevelFilter, FilterCriteria, FilterFieldFocus, LOG_NAMES,
    PanelFocus, PreviewViewMode, StatusDialog, TimeFilterOption,
};
use chrono::Local;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

#[cfg(target_os = "windows")]
use windows::Win32::System::EventLog::EvtClose;

impl AppState {
    /// Creates a new instance of AppState with default values.
    pub fn new() -> Self {
        let initial_log_name = LOG_NAMES[0].to_string();

        let log_file_path = Path::new("event_commander.log");
        let log_file_result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path);

        let log_file = match log_file_result {
            Ok(file) => Some(BufWriter::new(file)),
            Err(e) => {
                eprintln!(
                    "Failed to open or create log file '{}': {}. Logging disabled.",
                    log_file_path.display(),
                    e
                );
                None
            }
        };

        let app_state = AppState {
            focus: PanelFocus::Events,
            selected_log_index: 0,
            selected_log_name: initial_log_name,
            events: Vec::new(),
            table_state: TableState::default().with_selected(Some(0)),
            preview_scroll: 0,
            status_dialog: None,
            preview_event_id: None,
            preview_content: None,
            preview_raw_xml: None,
            preview_view_mode: PreviewViewMode::default(),
            log_file,
            #[cfg(target_os = "windows")]
            query_handle: None,
            #[cfg(target_os = "windows")]
            publisher_metadata_cache: HashMap::new(),
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
            filter_dialog_level: EventLevelFilter::default(),
            filter_dialog_time: TimeFilterOption::default(),
            available_sources: None,
            filter_dialog_source_input: String::new(),
            filter_dialog_filtered_sources: Vec::new(),
            filter_dialog_filtered_source_selection: None,
            filter_event_id_cursor: 0,
            filter_source_cursor: 0,
            search_cursor: 0,
            help_dialog_visible: false,
            help_scroll_position: 0,
        };

        app_state
    }

    /// Logs a message to the console and optionally to a file.
    pub fn log(&mut self, message: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_entry = format!("[{}]: {}\n", timestamp, message);
        if let Some(ref mut writer) = self.log_file {
            if let Err(e) = writer.write_all(log_entry.as_bytes()) {
                eprintln!("Error writing to log file: {}", e);
            }
        }
    }

    /// Displays an error message in a status dialog.
    pub fn show_error(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, true));
    }

    /// Displays a confirmation message in a status dialog.
    pub fn show_confirmation(&mut self, title: &str, message: &str) {
        self.status_dialog = Some(StatusDialog::new(title, message, false));
    }

    /// Gets the display name of the currently selected event level filter.
    pub fn get_current_level_name(&self) -> &str {
        self.active_filter
            .as_ref()
            .map_or(EventLevelFilter::All.display_name(), |f| {
                f.level.display_name()
            })
    }

    /// Gets a string indicating whether an advanced filter is active.
    pub fn get_filter_status(&self) -> &str {
        if self.active_filter.is_some() {
            "On"
        } else {
            "Off"
        }
    }

    /// Updates the preview panel content based on the current table selection.
    pub fn update_preview_for_selection(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if let Some(event) = self.events.get(selected_idx) {
                const MS_PREFIX: &str = "Microsoft-Windows-";
                let gray_style = Style::default().fg(Color::DarkGray);
                let default_style = Style::default();

                let source_spans = if event.provider_name_original.starts_with(MS_PREFIX) {
                    vec![
                        Span::styled(MS_PREFIX.to_string(), gray_style),
                        Span::styled(
                            event.provider_name_original[MS_PREFIX.len()..].to_string(),
                            default_style,
                        ),
                    ]
                } else {
                    vec![Span::styled(
                        event.provider_name_original.clone(),
                        default_style,
                    )]
                };
                let _source_line = Line::from(source_spans);

                let header_lines: Vec<Line> = vec![
                    Line::from(format!("Level:       {}", event.level)),
                    Line::from(format!("DateTime:    {}", event.datetime)),
                    Line::from(format!("Source:      {}", event.source)),
                    Line::from(format!("Event ID:    {}", event.id)),
                    Line::from(String::new()),
                    Line::from("--- Message ---".to_string()),
                ];

                let final_message_string = event
                    .formatted_message
                    .as_ref()
                    .filter(|fm| !fm.is_empty())
                    .cloned()
                    .unwrap_or_else(|| {
                        if !event.message.is_empty() && !event.message.starts_with("<No") {
                            event.message.clone()
                        } else {
                            "<No message content found>".to_string()
                        }
                    });

                let mut content_lines = header_lines;
                content_lines.extend(
                    final_message_string
                        .lines()
                        .map(|s| Line::from(s.to_string())),
                );

                let content_text = Text::from(content_lines);

                self.preview_event_id = Some(format!("{}_{}", event.source, event.id));
                self.preview_content = Some(content_text);
                self.preview_raw_xml = Some(event.raw_data.clone());
                self.preview_scroll = 0;
            } else {
                self.preview_event_id = None;
                self.preview_content = Some(Text::from(
                    "<Error: Selected index out of bounds>".to_string(),
                ));
                self.preview_raw_xml = None;
                self.preview_scroll = 0;
            }
        } else {
            self.preview_event_id = None;
            self.preview_content = Some(Text::from("<No event selected>".to_string()));
            self.preview_raw_xml = None;
            self.preview_scroll = 0;
        }
    }

    /// Scrolls down one event in the event list; loads more events if near the end.
    pub fn scroll_down(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.events.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        if !self.events.is_empty() {
            self.table_state.select(Some(i));
            self.update_preview_for_selection();
            if i >= self.events.len().saturating_sub(20) {
                #[cfg(target_os = "windows")]
                self.start_or_continue_log_load(false);
            }
        }
    }

    /// Scrolls up one event in the event list.
    pub fn scroll_up(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.events.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        if !self.events.is_empty() {
            self.table_state.select(Some(i));
            self.update_preview_for_selection();
        }
    }

    /// Scrolls down one page in the event list; loads more events if near the end.
    pub fn page_down(&mut self) {
        let page_size = 10;
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection =
            (current_selection + page_size).min(self.events.len().saturating_sub(1));
        if !self.events.is_empty() {
            self.table_state.select(Some(new_selection));
            self.update_preview_for_selection();
            if new_selection >= self.events.len().saturating_sub(20) {
                #[cfg(target_os = "windows")]
                self.start_or_continue_log_load(false);
            }
        }
    }

    /// Scrolls up one page in the event list.
    pub fn page_up(&mut self) {
        let page_size = 10;
        let current_selection = self.table_state.selected().unwrap_or(0);
        let new_selection = current_selection.saturating_sub(page_size);
        if !self.events.is_empty() {
            self.table_state.select(Some(new_selection));
            self.update_preview_for_selection();
        }
    }

    /// Selects the top event in the event list.
    pub fn go_to_top(&mut self) {
        if !self.events.is_empty() {
            self.table_state.select(Some(0));
            self.update_preview_for_selection();
        }
    }

    /// Selects the bottom event in the event list and loads more events if necessary.
    pub fn go_to_bottom(&mut self) {
        if !self.events.is_empty() {
            let last_index = self.events.len().saturating_sub(1);
            self.table_state.select(Some(last_index));
            self.update_preview_for_selection();
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(false);
        }
    }

    /// Cycles the focus among the Logs, Events, and Preview panels.
    pub fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Events => PanelFocus::Preview,
            PanelFocus::Preview => PanelFocus::Events,
        };
    }

    /// Scrolls the preview panel down by a specified number of lines.
    pub fn preview_scroll_down(&mut self, amount: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(amount as usize);
    }

    /// Scrolls the preview panel up by a specified number of lines.
    pub fn preview_scroll_up(&mut self, amount: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount as usize);
    }

    /// Scrolls the preview panel to the top.
    pub fn preview_go_to_top(&mut self) {
        self.preview_scroll = 0;
    }

    /// Scrolls the preview panel to the bottom.
    #[allow(dead_code)]
    pub fn preview_scroll_to_bottom(&mut self, content_height: usize, view_height: usize) {
        if content_height > view_height {
            self.preview_scroll = content_height - view_height;
        } else {
            self.preview_scroll = 0;
        }
    }

    /// Determines if an event matches the provided search term.
    pub fn event_matches_search(&self, event: &DisplayEvent, term_lower: &str) -> bool {
        event.message.to_lowercase().contains(term_lower)
            || event.source.to_lowercase().contains(term_lower)
            || event.level.to_lowercase().contains(term_lower)
            || event.id.to_lowercase().contains(term_lower)
            || event.datetime.to_lowercase().contains(term_lower)
    }

    /// Finds the next matching event based on the active search term.
    pub fn find_next_match(&mut self) -> Result<(), String> {
        if let Some(term) = self.last_search_term.clone() {
            let start_index = self.table_state.selected().map_or(0, |i| i + 1);
            for i in (start_index..self.events.len()).chain(0..start_index) {
                if let Some(event) = self.events.get(i) {
                    if self.event_matches_search(event, &term.to_lowercase()) {
                        self.table_state.select(Some(i));
                        self.update_preview_for_selection();
                        return Ok(());
                    }
                }
            }
            Err(format!("Search term '{}' not found.", term))
        } else {
            Err("No previous search term.".to_string())
        }
    }

    /// Finds the previous matching event based on the active search term.
    pub fn find_previous_match(&mut self) -> Result<(), String> {
        if let Some(term) = self.last_search_term.clone() {
            let start_index = self
                .table_state
                .selected()
                .map_or(self.events.len().saturating_sub(1), |i| i.saturating_sub(1));
            let end_index = self.events.len();
            for i in (0..=start_index)
                .rev()
                .chain((start_index + 1..end_index).rev())
            {
                if let Some(event) = self.events.get(i) {
                    if self.event_matches_search(event, &term.to_lowercase()) {
                        self.table_state.select(Some(i));
                        self.update_preview_for_selection();
                        return Ok(());
                    }
                }
            }
            Err(format!("Search term '{}' not found.", term))
        } else {
            Err("No previous search term.".to_string())
        }
    }

    /// Selects the selected log index and clears the active filter.
    pub fn select_log_index(&mut self, index: usize) {
        if index < crate::models::LOG_NAMES.len() {
            self.selected_log_index = index;
            self.selected_log_name = crate::models::LOG_NAMES[index].to_string();
            self.events.clear();
            self.table_state.select(Some(0));
            self.no_more_events = false;
            self.active_filter = None;
            #[cfg(target_os = "windows")]
            self.start_or_continue_log_load(true);
        }
    }

    /// Updates the filtered source list based on the filter dialog's input.
    pub fn update_filtered_sources(&mut self) {
        self.filter_dialog_filtered_sources.clear();
        if let Some(sources) = &self.available_sources {
            let input_lower = self.filter_dialog_source_input.to_lowercase();
            for (index, source) in sources.iter().enumerate() {
                if source.to_lowercase().contains(&input_lower) {
                    self.filter_dialog_filtered_sources
                        .push((index, source.clone()));
                }
            }
            if let Some(selected_pos) = self.filter_dialog_filtered_source_selection {
                if selected_pos >= self.filter_dialog_filtered_sources.len() {
                    self.filter_dialog_filtered_source_selection =
                        if self.filter_dialog_filtered_sources.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                }
            } else if !self.filter_dialog_filtered_sources.is_empty() {
                self.filter_dialog_filtered_source_selection = Some(0);
            }
            if let Some(selected_pos) = self.filter_dialog_filtered_source_selection {
                if let Some((original_index, _)) =
                    self.filter_dialog_filtered_sources.get(selected_pos)
                {
                    self.filter_dialog_source_index = *original_index;
                }
            }
        }
    }

    /// Updates the level filter in the active filter or creates a new filter with just the level
    pub fn update_level_filter(&mut self) {
        let current_filter = self.active_filter.take().unwrap_or_default();
        let new_level = current_filter.level.next();
        self.active_filter = Some(FilterCriteria {
            level: new_level,
            ..current_filter
        });
        #[cfg(target_os = "windows")]
        self.start_or_continue_log_load(true);
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(handle) = self.query_handle.take() {
                unsafe {
                    let _ = EvtClose(handle);
                }
            }
            for (_provider, handle) in self.publisher_metadata_cache.drain() {
                unsafe {
                    let _ = EvtClose(handle);
                }
            }
        }
        if let Some(mut writer) = self.log_file.take() {
            if let Err(e) = writer.flush() {
                eprintln!("Error flushing log file on drop: {}", e);
            }
        }
    }
}
