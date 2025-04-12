
#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError},
    Win32::System::EventLog::{
        EVT_HANDLE, EVT_VARIANT, EvtClose, EvtFormatMessage, EvtFormatMessageXml, EvtNext,
        EvtNextPublisherId, EvtOpenPublisherEnum, EvtOpenPublisherMetadata, EvtQuery,
        EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender, EvtRenderEventXml,
    },
    core::PCWSTR,
};

use crate::event_parser::parse_event_xml;
use crate::models::{AppState, EventLevelFilter, LOG_NAMES};

#[cfg(target_os = "windows")]
pub fn to_wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
pub fn render_event_xml(event_handle: EVT_HANDLE) -> Option<String> {
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
            let actual_len = buffer[..buffer_used as usize]
                .iter()
                .rposition(|&c| c == b'>' as u16)
                .map_or(buffer_used as usize, |p| p + 1);

            Some(String::from_utf16_lossy(&buffer[..actual_len]))
        } else {
            None
        }
    }
}

/// Loads available event log sources using the Windows Event Log API.
#[cfg(target_os = "windows")]
pub fn load_available_sources(app: &mut AppState) -> Option<Vec<String>> {
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
        sources.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        Some(sources)
    }
}

/// Starts or continues loading event logs using the Windows Event Log API.
#[cfg(target_os = "windows")]
impl AppState {
    pub fn start_or_continue_log_load(&mut self, initial_load: bool) {
        if self.is_loading || (!initial_load && self.no_more_events) {
            return;
        }
        self.is_loading = true;

        if initial_load {
            self.events.clear();
            self.table_state = ratatui::widgets::TableState::default();
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
                        vec![EVT_HANDLE::default(); crate::models::EVENT_BATCH_SIZE];
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
                            let mut display_event = parse_event_xml(&xml);

                            display_event.formatted_message = format_event_message(
                                self,
                                &display_event.provider_name_original,
                                event_handle,
                            );
                            self.events.push(display_event);
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

        self.update_preview_for_selection();

        self.is_loading = false;
    }

    pub fn build_xpath_from_filter(&self) -> String {
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

            if let Some(start_time_utc) = filter.time_filter.get_start_time() {
                let timestamp_str =
                    start_time_utc.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                conditions.push(format!(
                    "System/TimeCreated[@SystemTime >= '{}']",
                    timestamp_str
                ));
            }

            if conditions.is_empty() {
                "*".to_string()
            } else {
                format!("*[{}]", conditions.join(" and "))
            }
        } else {
            "*".to_string()
        }
    }
}

#[cfg(target_os = "windows")]
pub fn format_event_message(
    app_state: &mut AppState,
    provider_name_original: &str,
    event_handle: EVT_HANDLE,
) -> Option<String> {
    let provider_key = provider_name_original.to_string();
    let mut publisher_metadata: Option<EVT_HANDLE> = None;
    let evt_variants_slice: Option<&[EVT_VARIANT]> = None;

    unsafe {
        if let Some(cached_handle) = app_state.publisher_metadata_cache.get(&provider_key) {
            publisher_metadata = Some(*cached_handle);
        } else {
            match EvtOpenPublisherMetadata(
                None,
                PCWSTR::from_raw(to_wide_string(provider_name_original).as_ptr()),
                None,
                0,
                0,
            ) {
                Ok(handle) if !handle.is_invalid() => {
                    publisher_metadata = Some(handle);
                    app_state
                        .publisher_metadata_cache
                        .insert(provider_key.clone(), handle);
                }
                Ok(invalid_handle) => {
                    if !invalid_handle.is_invalid() {
                        let _ = EvtClose(invalid_handle);
                    }
                }
                Err(_) => {}
            }
        }
        if let Some(handle_to_use) = publisher_metadata {
            let mut final_formatted_message: Option<String> = None;
            let mut buffer_size_needed: u32 = 0;

            let flags_xml = EvtFormatMessageXml.0;
            let format_result_xml_size = EvtFormatMessage(
                handle_to_use,
                event_handle,
                0,
                evt_variants_slice,
                flags_xml,
                None,
                &mut buffer_size_needed,
            );

            match format_result_xml_size {
                Err(ref e) if e.code() == ERROR_INSUFFICIENT_BUFFER.into() => {
                    if buffer_size_needed > 0 {
                        let mut buffer: Vec<u16> = vec![0; buffer_size_needed as usize];
                        let format_result_xml_fill = EvtFormatMessage(
                            handle_to_use,
                            event_handle,
                            0,
                            evt_variants_slice,
                            flags_xml,
                            Some(buffer.as_mut_slice()),
                            &mut buffer_size_needed,
                        );
                        if format_result_xml_fill.is_ok() {
                            let null_pos =
                                buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                            let msg = String::from_utf16_lossy(&buffer[..null_pos]);
                            let trimmed_msg = msg.trim();
                            if !trimmed_msg.is_empty() && !trimmed_msg.starts_with('<') {
                                final_formatted_message = Some(trimmed_msg.to_string());
                            } else {
                            }
                        } else {
                        }
                    } else {
                    }
                }
                Err(_) => {}
                Ok(_) => {}
            }

            if final_formatted_message.is_none() {
                buffer_size_needed = 0;
                let flags_event = windows::Win32::System::EventLog::EvtFormatMessageEvent.0;
                let format_result_event_size = EvtFormatMessage(
                    handle_to_use,
                    event_handle,
                    0,
                    evt_variants_slice,
                    flags_event,
                    None,
                    &mut buffer_size_needed,
                );

                match format_result_event_size {
                    Err(ref e) if e.code() == ERROR_INSUFFICIENT_BUFFER.into() => {
                        if buffer_size_needed > 0 {
                            let mut buffer: Vec<u16> = vec![0; buffer_size_needed as usize];
                            let format_result_event_fill = EvtFormatMessage(
                                handle_to_use,
                                event_handle,
                                0,
                                evt_variants_slice,
                                flags_event,
                                Some(buffer.as_mut_slice()),
                                &mut buffer_size_needed,
                            );
                            if format_result_event_fill.is_ok() {
                                let null_pos =
                                    buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                                let msg = String::from_utf16_lossy(&buffer[..null_pos]);
                                let trimmed_msg = msg.trim();
                                if !trimmed_msg.is_empty() {
                                    final_formatted_message = Some(trimmed_msg.to_string());
                                } else {
                                }
                            } else {
                            }
                        } else {
                        }
                    }
                    Err(_) => {}
                    Ok(_) => {}
                }
            }

            final_formatted_message
        } else {
            None
        }
    }
}
