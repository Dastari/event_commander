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

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);

    let mut buf = Vec::new();
    let mut inside_system = false;
    let mut inside_event_id = false;
    let mut inside_level = false;

    let mut event_data_values = Vec::new();
    let mut current_text_buffer = String::new();
    let mut inside_event_or_user_data = false;

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
                    "EventData" | "UserData" => {
                        inside_event_or_user_data = true;
                        current_text_buffer.clear();
                    }
                    "Data" if inside_event_or_user_data => {
                        current_text_buffer.clear();
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
                    "EventData" | "UserData" => {
                        let trimmed_text = current_text_buffer.trim();
                        if !trimmed_text.is_empty() && event_data_values.is_empty() {
                            event_data_values.push(trimmed_text.to_string());
                        }
                        current_text_buffer.clear();
                        inside_event_or_user_data = false;
                    }
                    "Data" if inside_event_or_user_data => {
                        let trimmed_text = current_text_buffer.trim();
                        if !trimmed_text.is_empty() {
                            event_data_values.push(trimmed_text.to_string());
                        }
                        current_text_buffer.clear();
                    }
                    _ => {},
                }
            }
            Ok(Event::Text(ref e)) => {
                let text_result = e.unescape();
                if let Ok(text) = text_result {
                     let text_str = text.to_string();
                    if inside_event_id {
                        id = text_str;
                    } else if inside_level {
                        level = match text_str.as_str() {
                            "1" => "Critical".to_string(),
                            "2" => "Error".to_string(),
                            "3" => "Warning".to_string(),
                            "0" | "4" => "Information".to_string(),
                            "5" => "Verbose".to_string(),
                            _ => format!("Unknown({})", text_str),
                        };
                    } else if inside_event_or_user_data {
                        current_text_buffer.push_str(&text_str);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let final_message = if provider_name_original == "Microsoft-Windows-Windows Error Reporting" && id == "1001" {
        if !event_data_values.is_empty() {
             event_data_values.join("\n")
        } else {
            "<WER event data found but failed to parse/format>".to_string()
        }
    } else if !event_data_values.is_empty() {
        event_data_values.join("\n")
    } else {
        "<No relevant event data found>".to_string()
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