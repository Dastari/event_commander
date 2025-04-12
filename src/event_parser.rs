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

    // Variables for parsing EventData/UserData
    let mut event_data_values = Vec::new(); // To store individual <Data> or text nodes
    let mut current_text_buffer = String::new(); // Accumulate text between tags
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
                        current_text_buffer.clear(); // Clear buffer at the start of the section
                    }
                    "Data" if inside_event_or_user_data => {
                        // Clear buffer specifically for each Data tag start
                        current_text_buffer.clear();
                        // Removed WER attribute parsing here
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
                        // Capture any trailing text directly within EventData/UserData
                        let trimmed_text = current_text_buffer.trim();
                        if !trimmed_text.is_empty() && event_data_values.is_empty() {
                            // Only add if no <Data> tags were processed
                            event_data_values.push(trimmed_text.to_string());
                        }
                        current_text_buffer.clear();
                        inside_event_or_user_data = false;
                    }
                    "Data" if inside_event_or_user_data => {
                        // Process accumulated text when </Data> is encountered
                        let trimmed_text = current_text_buffer.trim();
                        if !trimmed_text.is_empty() {
                            event_data_values.push(trimmed_text.to_string());
                        }
                        current_text_buffer.clear(); // Clear after processing
                        // Removed WER map insertion here
                    }
                    _ => {},
                }
            }
            Ok(Event::Text(ref e)) => {
                let text_result = e.unescape();
                if let Ok(text) = text_result {
                     let text_str = text.to_string(); // Convert Cow<str> to String
                    if inside_event_id {
                        id = text_str;
                    } else if inside_level {
                        level = match text_str.as_str() { // Use text_str here
                            "1" => "Critical".to_string(),
                            "2" => "Error".to_string(),
                            "3" => "Warning".to_string(),
                            "0" | "4" => "Information".to_string(),
                            "5" => "Verbose".to_string(),
                            _ => format!("Unknown({})", text_str),
                        };
                    } else if inside_event_or_user_data {
                        // Append text content if inside EventData/UserData/Data
                        current_text_buffer.push_str(&text_str);
                        // Removed WER value accumulation here
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // --- Second Pass: Extract and process XML fragment after </System> ---
    // This second pass might be redundant now that we capture EventData/UserData in the first pass.
    // Let's comment it out for now and rely on the first pass extraction.
    /*
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
    */
    // Construct the fallback message string from the collected values
    let final_message = if provider_name_original == "Microsoft-Windows-Windows Error Reporting" && id == "1001" {
        // Attempt WER formatting using the extracted values. Needs a way to reconstruct the map or pass values.
        // For now, just join the values like other events.
        // TODO: Re-implement WER-specific formatting if needed, potentially requiring
        //       parsing the Name attribute of Data tags again or a different approach.
        if !event_data_values.is_empty() {
             event_data_values.join("\n")
        } else {
            "<WER event data found but failed to parse/format>".to_string()
        }
    } else if !event_data_values.is_empty() {
        event_data_values.join("\n") // Join extracted values for fallback message
    } else {
        "<No relevant event data found>".to_string()
    };

    // --- Temporary Debug Logging ---
    /* eprintln!(
        "[Parser Debug] EventID: {}, Provider: {}, DataValues: {:?}, FinalMessage: {:?}", 
        id, provider_name_original, event_data_values, final_message
    ); */
    // --- End Temporary Debug Logging ---

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