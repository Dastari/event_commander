use crossterm::event::{self, KeyCode, KeyEventKind};
use crate::models::{AppState, FilterFieldFocus, PanelFocus, PostKeyPressAction, Navigable};
use crate::helpers;
use std::fs;

/// Processes a key press event, updates the application state, and returns a PostKeyPressAction.
pub fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    if app_state.help_dialog_visible {
        return handle_help_dialog_keys(key, app_state);
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
        return handle_search_keys(key, app_state);
    }
    
    let mut status_action = PostKeyPressAction::None;
    
    if let Some(dialog) = &mut app_state.event_details_dialog {
        if dialog.visible {
            return handle_event_details_dialog_keys(key, dialog);
        }
    }
    
    if app_state.is_filter_dialog_visible {
        return handle_filter_dialog_keys(key, app_state);
    }
    
    match app_state.focus {
        PanelFocus::Logs => handle_logs_panel_keys(key, app_state),
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

fn handle_event_details_dialog_keys(
    key: event::KeyEvent, 
    dialog: &mut crate::models::EventDetailsDialog,
) -> PostKeyPressAction {
    let mut status_action = PostKeyPressAction::None;
    
    match key.code {
        KeyCode::Esc => {
            dialog.dismiss();
        }
        KeyCode::Char('v') => {
            dialog.toggle_view();
        }
        KeyCode::Char('s') => {
            let filename = format!(
                "{}-{}-{}-{}.xml",
                helpers::sanitize_filename(&dialog.log_name),
                helpers::sanitize_filename(&dialog.event_id),
                dialog.event_datetime.replace(':', "-").replace(' ', "_"),
                helpers::sanitize_filename(&dialog.event_source)
            );
            match helpers::pretty_print_xml(&dialog.raw_xml) {
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
        }
        KeyCode::Down => {
            dialog.scroll_down(dialog.current_visible_height);
        }
        KeyCode::PageUp => {
            dialog.page_up();
        }
        KeyCode::PageDown => {
            dialog.page_down(dialog.current_visible_height);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            dialog.go_to_top();
        }
        KeyCode::End | KeyCode::Char('G') => {
            dialog.go_to_bottom(dialog.current_visible_height);
        }
        _ => {}
    }
    
    status_action
}

fn handle_filter_dialog_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
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

fn handle_logs_panel_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    match key.code {
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
    }
    
    PostKeyPressAction::None
}

fn handle_events_panel_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    match key.code {
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
            app_state.update_level_filter();
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
    }
    
    PostKeyPressAction::None
}

fn handle_preview_panel_keys(key: event::KeyEvent, app_state: &mut AppState) -> PostKeyPressAction {
    match key.code {
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
    }
    
    PostKeyPressAction::None
} 