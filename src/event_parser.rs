use chrono::Local;
use quick_xml::{events::Event, Reader};
use crate::models::DisplayEvent;
use crate::event_api::format_wer_event_data_from_map;
use std::collections::HashMap;

/// Parses an event XML string and returns a DisplayEvent struct with extracted data.
#[cfg(target_os = "windows")]
pub fn parse_event_xml(xml: &str) -> DisplayEvent {
    let mut source = "<Parse Error>".to_string();
    let mut provider_name_original = "<Parse Error>".to_string();
    let mut id = "0".to_string();
    let mut level = "Unknown".to_string();
    let mut datetime = String::new();
    let mut system_data_end_pos: Option<usize> = None;
    let mut event_data_message = "<No event data found>".to_string();

    // --- First Pass: Extract System Info and find end of </System> tag ---
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);

    let mut buf = Vec::new();
    let mut inside_system = false;
    let mut inside_event_id = false;
    let mut inside_level = false;

    // Specific handling for WER data map
    let mut wer_data_map = HashMap::new();
    let mut inside_event_data_for_wer = false;
    let mut current_data_name = None;
    let mut current_data_value = String::new();
    let mut inside_data_for_wer = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local_name = std::str::from_utf8(e.name().local_name().into_inner())
                    .unwrap_or("")
                    .to_string();

                match local_name.as_str() {
                    "System" => inside_system = true,
                    "Provider" if inside_system => {
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key = std::str::from_utf8(attr.key.local_name().into_inner()).unwrap_or("");
                                if attr_key == "Name" {
                                    provider_name_original = attr.unescape_value().unwrap_or_default().to_string();
                                    source = provider_name_original.clone();
                                }
                            }
                        }
                    }
                    "EventID" if inside_system => inside_event_id = true,
                    "Level" if inside_system => inside_level = true,
                    "TimeCreated" if inside_system => {
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key = std::str::from_utf8(attr.key.local_name().into_inner()).unwrap_or("");
                                if attr_key == "SystemTime" {
                                    let time_str = attr.unescape_value().unwrap_or_default().to_string();
                                    datetime = chrono::DateTime::parse_from_rfc3339(&time_str)
                                        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
                                        .unwrap_or(time_str);
                                }
                            }
                        }
                    }
                    "EventData" => {
                        inside_event_data_for_wer = true;
                    }
                    "Data" if inside_event_data_for_wer => {
                        inside_data_for_wer = true;
                        current_data_value.clear();
                        current_data_name = None;
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key = std::str::from_utf8(attr.key.local_name().into_inner()).unwrap_or("");
                                if attr_key == "Name" {
                                    current_data_name = Some(attr.unescape_value().unwrap_or_default().to_string());
                                }
                            }
                        }
                    }
                    _ => {},
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = std::str::from_utf8(e.name().local_name().into_inner())
                    .unwrap_or("")
                    .to_string();
                match local_name.as_str() {
                    "System" => {
                        inside_system = false;
                        system_data_end_pos = Some(reader.buffer_position());
                    }
                    "EventID" => inside_event_id = false,
                    "Level" => inside_level = false,
                    "EventData" => {
                        inside_event_data_for_wer = false;
                    }
                    "Data" if inside_data_for_wer => {
                        inside_data_for_wer = false;
                        if let Some(name) = current_data_name.take() {
                            wer_data_map.insert(name, current_data_value.clone());
                        }
                    }
                    _ => {},
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if inside_event_id {
                    id = text;
                } else if inside_level {
                    level = match text.as_str() {
                        "1" => "Critical".to_string(),
                        "2" => "Error".to_string(),
                        "3" => "Warning".to_string(),
                        "0" | "4" => "Information".to_string(),
                        "5" => "Verbose".to_string(),
                        _ => format!("Unknown({})", text),
                    };
                } else if inside_data_for_wer {
                     current_data_value.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // --- Second Pass: Extract and process XML fragment after </System> --- 
    if let Some(start_pos) = system_data_end_pos {
        if let Some(end_pos) = xml.rfind("</Event>") {
            if end_pos > start_pos {
                let data_slice = &xml[start_pos..end_pos];
                let trimmed_fragment = data_slice.trim();
                if !trimmed_fragment.is_empty() {
                    let mut text_reader = Reader::from_str(trimmed_fragment);
                    text_reader.trim_text(true);
                    text_reader.expand_empty_elements(true);
                    let mut fragment_buf = Vec::new();
                    let mut extracted_texts = Vec::new();
                    loop {
                        match text_reader.read_event_into(&mut fragment_buf) {
                            Ok(Event::Text(e)) => {
                                if let Ok(text) = e.unescape() {
                                    let text_str = text.to_string();
                                    let trimmed_text = text_str.trim();
                                    if !trimmed_text.is_empty() {
                                        extracted_texts.push(trimmed_text.to_string());
                                    }
                                }
                            }
                            Ok(Event::Eof) => break,
                            Err(_) => break,
                            _ => {}
                        }
                        fragment_buf.clear();
                    }
                    if !extracted_texts.is_empty() {
                       event_data_message = extracted_texts.join("\n");
                    }
                }
            }
        }
    }

    // Commented out source name cleanup
    // if source != "<Parse Error>" && source.starts_with("Microsoft-Windows-") {
    //     source = source.trim_start_matches("Microsoft-Windows-").to_string();
    // }

    // WER Formatting check
    let final_message = if provider_name_original == "Microsoft-Windows-Windows Error Reporting" && id == "1001" && !wer_data_map.is_empty() {
        format_wer_event_data_from_map(&wer_data_map)
    } else {
        event_data_message
    };

    DisplayEvent {
        level,
        datetime,
        source,
        provider_name_original,
        id,
        message: final_message,
        raw_data: xml.to_string(),
        formatted_message: None,
    }
} 