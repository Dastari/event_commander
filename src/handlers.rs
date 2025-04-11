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
    match key.code {
        KeyCode::Esc => {
            app_state.is_searching = false;
            app_state.search_term.clear();
            app_state.last_search_term = None;
        }
        KeyCode::Enter => {
            if !app_state.search_term.is_empty() {
                app_state.is_searching = false;
                app_state.last_search_term = Some(app_state.search_term.clone());
                let _result = app_state.find_next_match();
                app_state.search_term.clear();
            } else {
                app_state.is_searching = false;
                app_state.search_term.clear();
                app_state.last_search_term = None;
            }
        }
        KeyCode::Char(c) => {
            app_state.search_term.push(c);
        }
        KeyCode::Backspace => {
            app_state.search_term.pop();
        }
        _ => {}
    }
    PostKeyPressAction::None
}

fn handle_filter_dialog_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
  
    match key.code {
        KeyCode::Esc => {
            app_state.is_filter_dialog_visible = false;
            return PostKeyPressAction::None;
        }
        KeyCode::Tab => {
            app_state.filter_dialog_focus = app_state.filter_dialog_focus.next();
        }
        KeyCode::BackTab => {
            app_state.filter_dialog_focus = app_state.filter_dialog_focus.previous();
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
                let criteria = crate::models::FilterCriteria {
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
                    && criteria.level == crate::models::EventLevelFilter::All
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
    
    PostKeyPressAction::None
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
            app_state.preview_scroll_down(usize::MAX as u16 / 2);
        }
        _ => {}
    }
    PostKeyPressAction::None
}