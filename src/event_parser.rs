use chrono::Local;
use quick_xml::Reader;
use crate::models::DisplayEvent;
use crate::event_api::format_wer_event_data_from_map;

/// Parses an event XML string and returns a DisplayEvent struct with extracted data.
#[cfg(target_os = "windows")]
pub fn parse_event_xml(xml: &str) -> DisplayEvent {
    let mut source = "<Parse Error>".to_string();
    let mut id = "0".to_string();
    let mut level = "Unknown".to_string();
    let mut datetime = String::new();

    // Create a quick-xml reader to process the XML
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);

    let mut buf = Vec::new();
    let mut inside_system = false;
    let mut inside_event_data = false;

    // Track if we're inside a particular element
    let mut inside_event_id = false;
    let mut inside_level = false;

    // We'll collect event data as we go
    let mut data_map = std::collections::HashMap::new();
    let mut current_data_name = None;
    let mut current_data_value = String::new();
    let mut inside_data = false;

    // Process the XML
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let local_name = std::str::from_utf8(e.name().local_name().into_inner())
                    .unwrap_or("")
                    .to_string();

                match local_name.as_str() {
                    "System" => {
                        inside_system = true;
                    }
                    "Provider" if inside_system => {
                        // Extract Provider Name attribute
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key =
                                    std::str::from_utf8(attr.key.local_name().into_inner())
                                        .unwrap_or("")
                                        .to_string();
                                if attr_key == "Name" {
                                    let value =
                                        attr.unescape_value().unwrap_or_default().to_string();
                                    source = value;
                                }
                            }
                        }
                    }
                    "EventID" if inside_system => {
                        inside_event_id = true;
                    }
                    "Level" if inside_system => {
                        inside_level = true;
                    }
                    "TimeCreated" if inside_system => {
                        // Extract SystemTime attribute
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key =
                                    std::str::from_utf8(attr.key.local_name().into_inner())
                                        .unwrap_or("")
                                        .to_string();
                                if attr_key == "SystemTime" {
                                    let time_str =
                                        attr.unescape_value().unwrap_or_default().to_string();
                                    datetime = chrono::DateTime::parse_from_rfc3339(&time_str)
                                        .map(|dt| {
                                            dt.with_timezone(&Local)
                                                .format("%Y-%m-%d %H:%M:%S")
                                                .to_string()
                                        })
                                        .unwrap_or(time_str);
                                }
                            }
                        }
                    }
                    "EventData" => {
                        inside_event_data = true;
                    }
                    "Data" if inside_event_data => {
                        inside_data = true;
                        current_data_value.clear();

                        // Check for a Name attribute
                        current_data_name = None;
                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let attr_key =
                                    std::str::from_utf8(attr.key.local_name().into_inner())
                                        .unwrap_or("")
                                        .to_string();
                                if attr_key == "Name" {
                                    let value =
                                        attr.unescape_value().unwrap_or_default().to_string();
                                    current_data_name = Some(value);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let local_name = std::str::from_utf8(e.name().local_name().into_inner())
                    .unwrap_or("")
                    .to_string();

                match local_name.as_str() {
                    "System" => {
                        inside_system = false;
                    }
                    "EventID" => {
                        inside_event_id = false;
                    }
                    "Level" => {
                        inside_level = false;
                    }
                    "EventData" => {
                        inside_event_data = false;
                    }
                    "Data" if inside_event_data => {
                        inside_data = false;
                        if let Some(name) = current_data_name.take() {
                            data_map.insert(name, current_data_value.clone());
                        }
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
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
                } else if inside_data {
                    current_data_value.push_str(&text);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }

        buf.clear();
    }

    // If we found the provider name, check if it starts with Microsoft-Windows-
    if source != "<Parse Error>" && source.starts_with("Microsoft-Windows-") {
        source = source.trim_start_matches("Microsoft-Windows-").to_string();
    }

    // Format message from event data
    let message = if !data_map.is_empty() {
        if source == "Windows Error Reporting" && id == "1001" {
            // Special handling for Windows Error Reporting events
            format_wer_event_data_from_map(&data_map)
        } else {
            // Generic format for other events
            data_map
                .iter()
                .map(|(name, value)| {
                    if value.is_empty() {
                        format!("  {}", name)
                    } else {
                        format!("  {}: {}", name, value)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if inside_event_data {
        "<No Data found>".to_string()
    } else {
        "<No EventData found>".to_string()
    };

    DisplayEvent {
        level,
        datetime,
        source,
        id,
        message,
        raw_data: xml.to_string(),
    }
} 