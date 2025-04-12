use crossterm::event::{self, KeyCode};
use crate::models::{AppState, FilterFieldFocus, PanelFocus, PostKeyPressAction, LOG_NAMES, PreviewViewMode};
use crate::helpers;
use std::fs;

/// Processes a key press event, updates the application state, and returns a PostKeyPressAction.
pub fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    // --- Early returns for dialogs ---
    if app_state.help_dialog_visible {
        return handle_help_dialog_keys(key, app_state);
    }

    if let Some(dialog) = &mut app_state.status_dialog {
        if dialog.visible {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    dialog.dismiss();
                    app_state.log("Status dialog dismissed.");
                }
                _ => { /* Consume key */ }
            }
            return PostKeyPressAction::None; // Dialog handled the key
        }
    }

    if app_state.is_searching {
        return handle_search_keys(key, app_state); // Search handles its own keys
    }

    if app_state.is_filter_dialog_visible {
        return handle_filter_dialog_keys(key, app_state);
    }

    // --- Global keybindings (only if no dialogs handled input) ---
    match key.code {
        KeyCode::Char('q') => return PostKeyPressAction::Quit,
        KeyCode::F(1) => {
             app_state.help_dialog_visible = true;
             return PostKeyPressAction::None;
        }
        KeyCode::Char(c @ '1'..='5') => {
            if let Some(index) = c.to_digit(10).map(|d| d as usize - 1) {
                if index < LOG_NAMES.len() {
                    app_state.select_log_index(index);
                    return PostKeyPressAction::ReloadData;
                }
            }
             // If it's 1-5 but index is invalid, fall through to focus-based handling
        }
        KeyCode::Tab | KeyCode::Right => {
            app_state.switch_focus(); // Cycle between Events and Preview
            return PostKeyPressAction::None;
        }
        KeyCode::BackTab | KeyCode::Left => {
            if app_state.focus == PanelFocus::Preview {
                 app_state.focus = PanelFocus::Events;
            } else {
                app_state.switch_focus(); // Should cycle back from Events to Preview
            }
            return PostKeyPressAction::None;
        }
        _ => {} // Other keys fall through to focus-based handling
    }

    // --- Focus-based handling (Events & Preview only) ---
    // The result of this match is implicitly returned
    match app_state.focus {
        PanelFocus::Events => handle_events_panel_keys(key, app_state),
        PanelFocus::Preview => handle_preview_panel_keys(key, app_state),
    }
}

fn handle_help_dialog_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
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
    PostKeyPressAction::None
}

fn handle_search_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    let mut action = PostKeyPressAction::None;
    let text = &mut app_state.search_term;
    let cursor = &mut app_state.search_cursor;
    let mut perform_search = false;

    match key.code {
        KeyCode::Esc => {
            app_state.is_searching = false;
            text.clear();
            *cursor = 0;
            app_state.last_search_term = None;
        }
        KeyCode::Enter => {
            app_state.is_searching = false;
            if !text.is_empty() {
                app_state.last_search_term = Some(text.clone());
                perform_search = true; // Flag to search after borrow ends
            } else {
                app_state.last_search_term = None;
            }
            text.clear();
            *cursor = 0;
        }
        KeyCode::Char(c) => {
             if text.is_empty() {
                text.push(c);
                *cursor = 1;
            } else {
                // Find the byte index corresponding to the character cursor
                let byte_idx = text.char_indices().nth(*cursor).map(|(idx, _)| idx).unwrap_or(text.len());
                text.insert(byte_idx, c);
                *cursor = cursor.saturating_add(1);
            }
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                let char_idx_to_remove = *cursor - 1;
                // Find the byte index for the character to remove
                if let Some((byte_idx, _)) = text.char_indices().nth(char_idx_to_remove) {
                    text.remove(byte_idx);
                    *cursor = cursor.saturating_sub(1);
                }
            }
        }
        KeyCode::Delete => { // Handle delete key (forward delete)
            if *cursor < text.chars().count() {
                 // Find the byte index for the character at the cursor
                 if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                    text.remove(byte_idx);
                    // Cursor position doesn't change
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            *cursor = cursor.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Ensure cursor doesn't go beyond the number of characters
            *cursor = (*cursor + 1).min(text.chars().count());
        }
        KeyCode::Home | KeyCode::Char('0') => {
             *cursor = 0;
        }
        KeyCode::End | KeyCode::Char('$') => {
            *cursor = text.chars().count();
        }
        KeyCode::Char('x') => { // Delete character under cursor
            if *cursor < text.chars().count() {
                 // Find byte index for character at cursor
                 if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                    text.remove(byte_idx);
                    // Cursor position doesn't change, but cap at new length
                    *cursor = (*cursor).min(text.chars().count());
                }
            }
        }
         KeyCode::Char('D') => { // Delete from cursor to end of line
            if *cursor < text.chars().count() {
                 // Find byte index for character at cursor
                 if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                    text.truncate(byte_idx);
                    // Cursor remains at the same index (now end of string)
                     *cursor = (*cursor).min(text.chars().count()); // Cap cursor
                }
            } else {
                // If cursor is already at end, D does nothing
            }
        }
        _ => {}
    }
    
    // Perform search if flagged (after mutable borrows of text/cursor are released)
    if perform_search {
        let _result = app_state.find_next_match(); 
        // Handle result? Maybe set status? For now, ignore.
    }

    action
}

fn handle_filter_dialog_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    let mut action = PostKeyPressAction::None;
    let mut perform_reload = false;

    // Get mutable references to the relevant string and cursor based on focus
    let text_cursor_refs: (Option<&mut String>, Option<&mut usize>) = match app_state.filter_dialog_focus {
        FilterFieldFocus::EventId => (
            Some(&mut app_state.filter_dialog_event_id),
            Some(&mut app_state.filter_event_id_cursor),
        ),
        FilterFieldFocus::Source => (
            Some(&mut app_state.filter_dialog_source_input),
            Some(&mut app_state.filter_source_cursor),
        ),
        _ => (None, None), // For Level, Apply, Clear - no text input
    };

    // Only process text input keys if text and cursor references are Some
    if let (Some(text), Some(cursor)) = text_cursor_refs {
        match key.code {
            KeyCode::Char(c) => {
                if app_state.filter_dialog_focus == FilterFieldFocus::EventId && !c.is_ascii_digit() {
                    // Ignore non-digits for Event ID
                } else {
                     if text.is_empty() {
                        text.push(c);
                        *cursor = 1;
                    } else {
                        let byte_idx = text.char_indices().nth(*cursor).map(|(idx, _)| idx).unwrap_or(text.len());
                        text.insert(byte_idx, c);
                        *cursor = cursor.saturating_add(1);
                    }
                    if app_state.filter_dialog_focus == FilterFieldFocus::Source {
                        app_state.update_filtered_sources();
                    }
                }
            }
            KeyCode::Backspace => {
                if *cursor > 0 {
                    let char_idx_to_remove = *cursor - 1;
                    if let Some((byte_idx, _)) = text.char_indices().nth(char_idx_to_remove) {
                        text.remove(byte_idx);
                        *cursor = cursor.saturating_sub(1);
                        if app_state.filter_dialog_focus == FilterFieldFocus::Source {
                            app_state.update_filtered_sources();
                        }
                    }
                }
            }
            KeyCode::Delete => {
                if *cursor < text.chars().count() {
                    if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                        text.remove(byte_idx);
                         if app_state.filter_dialog_focus == FilterFieldFocus::Source {
                             app_state.update_filtered_sources(); // Update list after delete
                         }
                         // Cursor stays
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                *cursor = cursor.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                *cursor = (*cursor + 1).min(text.chars().count());
            }
            KeyCode::Home | KeyCode::Char('0') => {
                *cursor = 0;
            }
            KeyCode::End | KeyCode::Char('$') => {
                *cursor = text.chars().count();
            }
            KeyCode::Char('x') => {
                if *cursor < text.chars().count() {
                    if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                        text.remove(byte_idx);
                        *cursor = (*cursor).min(text.chars().count()); // Cap cursor
                         if app_state.filter_dialog_focus == FilterFieldFocus::Source {
                             app_state.update_filtered_sources();
                         }
                    }
                }
            }
            KeyCode::Char('D') => {
                if *cursor < text.chars().count() {
                    if let Some((byte_idx, _)) = text.char_indices().nth(*cursor) {
                        text.truncate(byte_idx);
                        *cursor = (*cursor).min(text.chars().count()); // Cap cursor
                         if app_state.filter_dialog_focus == FilterFieldFocus::Source {
                             app_state.update_filtered_sources();
                         }
                    }
                } else {
                    // D at end does nothing
                }
            }
            // Exclude Enter, Tab, Backtab, Esc, Arrows (Up/Down/Left/Right specific to Level/Source list)
            // These are handled outside this `if let` block or below
            _ => { /* Other keys ignored for text input */ }
        }
    }
    
    // --- Handle non-text input keys --- 
    match key.code {
        KeyCode::Esc => {
            app_state.is_filter_dialog_visible = false;
            // Reset cursor positions on close
            app_state.filter_event_id_cursor = 0;
            app_state.filter_source_cursor = 0;
            action = PostKeyPressAction::None;
        }
        KeyCode::Tab => {
            app_state.filter_dialog_focus = app_state.filter_dialog_focus.next();
            action = PostKeyPressAction::None;
        }
        KeyCode::BackTab => {
            app_state.filter_dialog_focus = app_state.filter_dialog_focus.previous();
            action = PostKeyPressAction::None;
        }
        KeyCode::Enter => match app_state.filter_dialog_focus {
            FilterFieldFocus::Source => {
                let input_trimmed = app_state.filter_dialog_source_input.trim();
                if input_trimmed.is_empty() {
                    app_state.filter_dialog_source_input.clear();
                } else {
                    if let Some(selected_pos) = app_state.filter_dialog_filtered_source_selection {
                        if let Some((_, name)) = app_state.filter_dialog_filtered_sources.get(selected_pos) {
                            app_state.filter_dialog_source_input = name.clone();
                        } else {
                            app_state.filter_dialog_source_input = input_trimmed.to_string();
                        }
                    } else {
                        app_state.filter_dialog_source_input = input_trimmed.to_string();
                    }
                }
                app_state.update_filtered_sources(); 
                app_state.filter_dialog_focus = FilterFieldFocus::Level;
                app_state.filter_source_cursor = app_state.filter_dialog_source_input.chars().count(); // Move cursor to end
            }
            FilterFieldFocus::EventId => {
                // Just trim and move focus, keep cursor at end
                app_state.filter_dialog_event_id = app_state.filter_dialog_event_id.trim().to_string();
                app_state.filter_event_id_cursor = app_state.filter_dialog_event_id.chars().count();
                app_state.filter_dialog_focus = FilterFieldFocus::Level;
            }
            FilterFieldFocus::Level => {
                app_state.filter_dialog_focus = FilterFieldFocus::Source; // Move to Source next
            }
            FilterFieldFocus::Apply => {
                let source_input_trimmed = app_state.filter_dialog_source_input.trim();
                let selected_source = if source_input_trimmed.is_empty() { None } else { Some(source_input_trimmed.to_string()) };
                let event_id_trimmed = app_state.filter_dialog_event_id.trim();
                let selected_event_id = if event_id_trimmed.is_empty() { None } else { Some(event_id_trimmed.to_string()) };
                
                let criteria = crate::models::FilterCriteria {
                    source: selected_source,
                    event_id: selected_event_id,
                    level: app_state.filter_dialog_level,
                };
                if criteria.source.is_none() && criteria.event_id.is_none() && criteria.level == crate::models::EventLevelFilter::All {
                    app_state.active_filter = None;
                } else {
                    app_state.active_filter = Some(criteria);
                }
                app_state.is_filter_dialog_visible = false;
                app_state.filter_event_id_cursor = 0; // Reset cursors
                app_state.filter_source_cursor = 0;
                perform_reload = true;
            }
            FilterFieldFocus::Clear => {
                app_state.active_filter = None;
                app_state.is_filter_dialog_visible = false;
                app_state.filter_event_id_cursor = 0; // Reset cursors
                app_state.filter_source_cursor = 0;
                perform_reload = true;
            }
        },
        KeyCode::Left => match app_state.filter_dialog_focus {
            // Left handled by text input logic above if focused on text field
            FilterFieldFocus::Level => {
                app_state.filter_dialog_level = app_state.filter_dialog_level.previous();
            }
             FilterFieldFocus::Apply | FilterFieldFocus::Clear => { // Allow Left/Right on buttons
                 app_state.filter_dialog_focus = app_state.filter_dialog_focus.previous();
             }
            _ => {}
        },
        KeyCode::Right => match app_state.filter_dialog_focus {
            // Right handled by text input logic above if focused on text field
            FilterFieldFocus::Level => {
                app_state.filter_dialog_level = app_state.filter_dialog_level.next();
            }
            FilterFieldFocus::Apply | FilterFieldFocus::Clear => { // Allow Left/Right on buttons
                 app_state.filter_dialog_focus = app_state.filter_dialog_focus.next();
            }
            _ => {}
        },
        KeyCode::Up => match app_state.filter_dialog_focus {
            FilterFieldFocus::Source => {
                if !app_state.filter_dialog_filtered_sources.is_empty() {
                    let count = app_state.filter_dialog_filtered_sources.len();
                    let current_pos = app_state.filter_dialog_filtered_source_selection.unwrap_or(0);
                    let new_pos = if current_pos == 0 { count - 1 } else { current_pos - 1 };
                    app_state.filter_dialog_filtered_source_selection = Some(new_pos);
                    if let Some((idx, name)) = app_state.filter_dialog_filtered_sources.get(new_pos) {
                        app_state.filter_dialog_source_input = name.clone();
                        app_state.filter_dialog_source_index = *idx;
                        app_state.filter_source_cursor = app_state.filter_dialog_source_input.chars().count(); // Update cursor
                    }
                }
            }
            _ => {} // Up arrow otherwise moves focus via BackTab
        },
        KeyCode::Down => match app_state.filter_dialog_focus {
            FilterFieldFocus::Source => {
                if !app_state.filter_dialog_filtered_sources.is_empty() {
                    let count = app_state.filter_dialog_filtered_sources.len();
                    let current_pos = app_state.filter_dialog_filtered_source_selection.unwrap_or(0);
                    let new_pos = if current_pos >= count - 1 { 0 } else { current_pos + 1 };
                    app_state.filter_dialog_filtered_source_selection = Some(new_pos);
                    if let Some((idx, name)) = app_state.filter_dialog_filtered_sources.get(new_pos) {
                        app_state.filter_dialog_source_input = name.clone();
                        app_state.filter_dialog_source_index = *idx;
                         app_state.filter_source_cursor = app_state.filter_dialog_source_input.chars().count(); // Update cursor
                    }
                }
            }
            _ => {} // Down arrow otherwise moves focus via Tab
        },
        // Default: Check if it's a text input key not handled above
        // (This is implicitly covered by the `if let (Some(text), Some(cursor)) = ...` block now)
        _ => {} 
    }
    
    if perform_reload {
        action = PostKeyPressAction::ReloadData;
    }

    action
}

fn handle_events_panel_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    match key.code {
        KeyCode::Down => app_state.scroll_down(),
        KeyCode::Up => app_state.scroll_up(),
        KeyCode::PageDown => app_state.page_down(),
        KeyCode::PageUp => app_state.page_up(),
        KeyCode::Home | KeyCode::Char('g') => app_state.go_to_top(),
        KeyCode::End | KeyCode::Char('G') => app_state.go_to_bottom(),
        KeyCode::Char('s') => {
            app_state.sort_descending = !app_state.sort_descending;
            return PostKeyPressAction::ReloadData;
        }
        KeyCode::Char('l') => {
            app_state.update_level_filter();
            return PostKeyPressAction::ReloadData;
        }
        KeyCode::Char('f') => {
            return PostKeyPressAction::OpenFilterDialog;
        }
        KeyCode::Char('/') => {
            if let Some(last_search) = &app_state.last_search_term {
                app_state.search_term = last_search.clone();
            }
            app_state.is_searching = true;
        }
        KeyCode::Char('n') => {
            match app_state.find_next_match() {
                Ok(_) => {},
                Err(msg) => return PostKeyPressAction::ShowConfirmation("Search Failed".to_string(), msg),
            }
        }
        KeyCode::Char('p') => {
            match app_state.find_previous_match() {
                 Ok(_) => {},
                 Err(msg) => return PostKeyPressAction::ShowConfirmation("Search Failed".to_string(), msg),
             }
        }
        KeyCode::Enter => {
            if app_state.table_state.selected().is_some() {
                app_state.focus = PanelFocus::Preview;
            } else {
                app_state.show_confirmation("No Selection", "Please select an event first.");
            }
        }
        _ => {}
    }
    PostKeyPressAction::None
}

fn handle_preview_panel_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    match key.code {
        KeyCode::Esc | KeyCode::Left => {
            app_state.focus = PanelFocus::Events;
        }
        KeyCode::Char('v') => {
            app_state.preview_view_mode = match app_state.preview_view_mode {
                PreviewViewMode::Formatted => PreviewViewMode::RawXml,
                PreviewViewMode::RawXml => PreviewViewMode::Formatted,
            };
            app_state.preview_scroll = 0;
        }
        KeyCode::Char('s') => {
            if let (Some(raw_xml), Some(event_id)) = (
                &app_state.preview_raw_xml,
                app_state.table_state.selected().and_then(|idx| app_state.events.get(idx)),
            ) {
                let xml_content = raw_xml.clone();
                let filename = format!(
                    "{}-{}-[{}]-{}.xml",
                    helpers::sanitize_filename(&app_state.selected_log_name),
                    event_id.datetime.replace(':', "-").replace(' ', "_"),
                    helpers::sanitize_filename(&event_id.id),
                    helpers::sanitize_filename(&event_id.source)
                );
                
                match helpers::pretty_print_xml(&xml_content) {
                    Ok(pretty_xml) => match fs::write(&filename, &pretty_xml) {
                        Ok(_) => {
                           return PostKeyPressAction::ShowConfirmation(
                                "Save Successful".to_string(),
                                format!("Event saved to:\n\n{}", filename),
                            );
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to save event to {}: {}", filename, e);
                            app_state.log(&format!("Save error: {}", e));
                            return PostKeyPressAction::ShowConfirmation("Save Failed".to_string(), err_msg);
                        }
                    },
                    Err(e) => {
                         app_state.log(&format!("Failed to pretty print XML for saving ({}). Saving raw.", e));
                         match fs::write(&filename, &xml_content) {
                            Ok(_) => {
                                return PostKeyPressAction::ShowConfirmation(
                                    "Save Successful (Raw)".to_string(),
                                    format!("Event saved (raw XML) to:\\n{}", filename),
                                );
                            }
                            Err(e) => {
                                let err_msg = format!("Failed to save raw event to {}: {}", filename, e);
                                app_state.log(&format!("Raw save error: {}", e));
                                return PostKeyPressAction::ShowConfirmation("Save Failed".to_string(), err_msg);
                            }
                        }
                    }
                }
            } else {
                return PostKeyPressAction::ShowConfirmation(
                    "Save Failed".to_string(),
                    "No event selected or raw XML data unavailable to save.".to_string(),
                );
            }
        }
        KeyCode::Down => app_state.preview_scroll_down(1),
        KeyCode::Up => app_state.preview_scroll_up(1),
        KeyCode::PageDown => app_state.preview_scroll_down(10),
        KeyCode::PageUp => app_state.preview_scroll_up(10),
        KeyCode::Home | KeyCode::Char('g') => app_state.preview_go_to_top(),
        KeyCode::End | KeyCode::Char('G') => { 
            // We don't know the exact bottom here, so scroll a large amount
            // The render logic will cap the scroll correctly.
            app_state.preview_scroll_down(u16::MAX); // Scroll max possible u16
        }
        _ => {}
    }
    PostKeyPressAction::None
}