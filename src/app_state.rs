use crate::models::{AppState, FilterCriteria, EventLevelFilter, PanelFocus, StatusDialog, EventDetailsDialog};

impl AppState {
    /// Switches to the next log in the list and clears the active filter.
    pub fn next_log(&mut self) {
        if self.selected_log_index < crate::models::LOG_NAMES.len() - 1 {
            self.selected_log_index += 1;
        }
        self.active_filter = None;
    }
    
    /// Switches to the previous log in the list and clears the active filter.
    pub fn previous_log(&mut self) {
        self.selected_log_index = self.selected_log_index.saturating_sub(1);
        self.active_filter = None;
    }
    
    /// Scrolls down one event in the event list; loads more events if near the end.
    pub fn scroll_down(&mut self) {
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
    pub fn scroll_up(&mut self) {
        if self.events.is_empty() {
            self.select_event(None);
            return;
        }
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.select_event(Some(i));
    }
    
    /// Scrolls down one page in the event list; loads more events if near the end.
    pub fn page_down(&mut self) {
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
    pub fn page_up(&mut self) {
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
    pub fn go_to_top(&mut self) {
        if !self.events.is_empty() {
            self.select_event(Some(0));
        }
    }
    
    /// Selects the bottom event in the event list and loads more events if necessary.
    pub fn go_to_bottom(&mut self) {
        if !self.events.is_empty() {
            let last_index = self.events.len().saturating_sub(1);
            self.select_event(Some(last_index));
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
    pub fn preview_scroll_down(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(lines);
    }
    
    /// Scrolls the preview panel up by a specified number of lines.
    pub fn preview_scroll_up(&mut self, lines: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(lines);
    }
    
    /// Scrolls the preview panel to the top.
    pub fn preview_go_to_top(&mut self) {
        self.preview_scroll = 0;
    }
    
    /// Resets the preview scroll position.
    pub fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
    }
    
    /// Selects an event by index in the event table and resets preview scroll.
    pub fn select_event(&mut self, index: Option<usize>) {
        self.table_state.select(index);
        self.reset_preview_scroll();
    }
    
    /// Determines if an event matches the provided search term.
    pub fn event_matches_search(&self, event: &crate::models::DisplayEvent, term_lower: &str) -> bool {
        event.level.to_lowercase().contains(term_lower)
            || event.datetime.to_lowercase().contains(term_lower)
            || event.source.to_lowercase().contains(term_lower)
            || event.id.to_lowercase().contains(term_lower)
            || event.message.to_lowercase().contains(term_lower)
    }
    
    /// Finds the next matching event based on the active search term.
    pub fn find_next_match(&mut self) -> bool {
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
    pub fn find_previous_match(&mut self) -> bool {
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
    
    /// Sets the selected log index and clears the active filter.
    pub fn select_log_index(&mut self, index: usize) {
        if index < crate::models::LOG_NAMES.len() {
            self.selected_log_index = index;
            self.active_filter = None; // Clear filter when changing logs
        }
    }
    
    /// Updates the filtered source list based on the filter dialog's input.
    pub fn update_filtered_sources(&mut self) {
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
    
    /// Updates the level filter in the active filter or creates a new filter with just the level
    pub fn update_level_filter(&mut self) {
        let next_level = match &self.active_filter {
            Some(filter) => filter.level.next(),
            None => EventLevelFilter::Information, // Start with Information if no filter
        };
        
        if next_level == EventLevelFilter::All {
            // If cycling back to All and no other filters, remove the filter entirely
            if let Some(filter) = &self.active_filter {
                if filter.source.is_none() && filter.event_id.is_none() {
                    self.active_filter = None;
                    return;
                }
            }
        }
        
        // Update existing filter or create new one with just the level
        if let Some(filter) = &mut self.active_filter {
            filter.level = next_level;
        } else if next_level != EventLevelFilter::All {
            self.active_filter = Some(FilterCriteria {
                source: None,
                event_id: None,
                level: next_level,
            });
        }
    }
    
    /// Gets the display name of the current filter level
    pub fn get_current_level_name(&self) -> &str {
        if let Some(filter) = &self.active_filter {
            filter.level.display_name()
        } else {
            "All"
        }
    }
    
    /// Returns the status of the current filter
    pub fn get_filter_status(&self) -> &str {
        if self.active_filter.is_some() {
            "Active"
        } else {
            "Inactive"
        }
    }
} 