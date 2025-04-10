mod app_state;
mod event_api;
mod event_parser;
mod handlers;
mod helpers;
mod models;
mod terminal;
mod ui;

use crossterm::event::{self, Event, KeyEventKind};
use models::PostKeyPressAction;
use std::{error::Error, time::Duration};

#[cfg(target_os = "windows")]
use windows::Win32::System::EventLog::EvtClose;

/// Application entry point; initializes the terminal and application state, and processes events.
fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = terminal::init_terminal()?;
    let mut app_state = models::AppState::new();
    
    #[cfg(target_os = "windows")]
    app_state.start_or_continue_log_load(true);
    
    loop {
        terminal.draw(|frame| ui::ui(frame, &mut app_state))?;
        
        let mut post_action = PostKeyPressAction::None;
        
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    post_action = handlers::handle_key_press(key, &mut app_state);
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
                        app_state.available_sources = event_api::load_available_sources(&mut app_state);
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
                    app_state.filter_dialog_level = models::EventLevelFilter::All;
                }
                app_state.update_filtered_sources();
                app_state.filter_dialog_focus = models::FilterFieldFocus::Source;
                app_state.is_filter_dialog_visible = true;
            }
            PostKeyPressAction::Quit => break,
            PostKeyPressAction::None => {}
        }
    }
    
    terminal::restore_terminal()?;
    Ok(())
}
