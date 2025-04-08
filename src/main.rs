use chrono::{Local, TimeZone, Utc};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState},
};
use std::{
    error::Error,
    io::{self, stdout, Stdout},
    time::Duration,
    vec,
};

#[cfg(target_os = "windows")]
use windows::{
    core::PCWSTR,
    Win32::Foundation::{GetLastError, ERROR_HANDLE_EOF, ERROR_INSUFFICIENT_BUFFER, HANDLE},
    Win32::System::EventLog::{
        CloseEventLog, OpenEventLogW, ReadEventLogW, EVENTLOGRECORD,
        EVENTLOG_SEQUENTIAL_READ, READ_EVENT_LOG_READ_FLAGS,
    },
};

// Define missing constant
#[cfg(target_os = "windows")]
const EVENTLOG_BACKWARDS_READ: u32 = 0x0008;
#[cfg(target_os = "windows")]
const EVENTLOG_ERROR_TYPE: u16 = 0x0001;
#[cfg(target_os = "windows")]
const EVENTLOG_WARNING_TYPE: u16 = 0x0002;
#[cfg(target_os = "windows")]
const EVENTLOG_INFORMATION_TYPE: u16 = 0x0004;
#[cfg(target_os = "windows")]
const EVENTLOG_AUDIT_SUCCESS: u16 = 0x0008;
#[cfg(target_os = "windows")]
const EVENTLOG_AUDIT_FAILURE: u16 = 0x0010;

// #[cfg(target_os = "windows")]
// mod windows_constants {
//     // These constants are for reference, we'll use the Windows API directly
//     pub const EVENTLOG_SEQUENTIAL_READ: u32 = 0x0001;
//     pub const EVENTLOG_BACKWARDS_READ: u32 = 0x0008;
//     pub const EVENTLOG_ERROR_TYPE: u16 = 0x0001;
//     pub const EVENTLOG_WARNING_TYPE: u16 = 0x0002;
//     pub const EVENTLOG_INFORMATION_TYPE: u16 = 0x0004;
//     pub const EVENTLOG_AUDIT_SUCCESS: u16 = 0x0008;
//     pub const EVENTLOG_AUDIT_FAILURE: u16 = 0x0010;
// }

// #[cfg(target_os = "windows")]
// use windows_constants::*;

#[cfg(not(target_os = "windows"))]
mod windows_stubs {
    #[derive(Debug)]
    pub struct HANDLE(pub isize);
    
    impl HANDLE {
        pub fn is_invalid(&self) -> bool {
            self.0 == -1
        }
    }
    
    impl Default for HANDLE {
        fn default() -> Self {
            HANDLE(-1)
        }
    }
    
    pub struct EVENTLOGRECORD {
        pub Length: u32,
        pub Reserved: u32,
        pub RecordNumber: u32,
        pub TimeGenerated: u32,
        pub TimeWritten: u32,
        pub EventID: u32,
        pub EventType: u16,
        pub NumStrings: u16,
        pub EventCategory: u16,
        pub ReservedFlags: u16,
        pub ClosingRecordNumber: u32,
        pub StringOffset: u32,
        pub UserSidLength: u32,
        pub UserSidOffset: u32,
        pub DataLength: u32,
        pub DataOffset: u32,
    }
}

#[cfg(not(target_os = "windows"))]
use windows_stubs::*;

// Constants for EventLog header size (missing from Windows API)
const EVENTLOG_HEADER_SIZE: usize = 0x38; // 56 bytes standard header size

// Helper function for Wide String Conversion
#[cfg(target_os = "windows")]
fn to_wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
fn to_wide_string(s: &str) -> Vec<u16> {
    // Just a stub implementation for non-Windows platforms
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Helper function for Event Type Mapping
fn map_event_type(event_type: u16) -> String {
    match event_type {
        1 => "Error".to_string(),        // EVENTLOG_ERROR_TYPE = 1u16
        2 => "Warning".to_string(),      // EVENTLOG_WARNING_TYPE = 2u16
        4 => "Information".to_string(),  // EVENTLOG_INFORMATION_TYPE = 4u16
        8 => "Audit Success".to_string(),// EVENTLOG_AUDIT_SUCCESS = 8u16
        16 => "Audit Failure".to_string(),// EVENTLOG_AUDIT_FAILURE = 16u16
        _ => format!("Unknown({})", event_type),
    }
}

// Define the standard event log names - use the exact names Windows expects
const LOG_NAMES: [&str; 5] = [
    "Application",  // Standard Application log
    "System",       // Standard System log
    "Security",     // Security log (may require elevated permissions)
    "Setup",        // Setup log
    "ForwardedEvents", // Forwarded events
];

// Define constants
const EVENT_BATCH_SIZE: usize = 100; // Number of events to load/show initially

// Define enums
#[derive(PartialEq, Debug)] // Added Debug for println!
enum PanelFocus {
    Logs,
    Events,
}

// Define structs
// Simplified structure to hold event data for display
#[derive(Clone, Debug)]
struct DisplayEvent {
    level: String,
    datetime: String,
    source: String,
    id: String,
    raw_data: String, // Add raw data field to store additional event information
}

// Custom dialog struct for displaying errors
#[derive(Debug, Clone)]
struct ErrorDialog {
    title: String,
    message: String,
    visible: bool,
}

// Custom dialog for displaying event details
#[derive(Debug, Clone)]
struct EventDetailsDialog {
    title: String,
    content: String,
    visible: bool,
    scroll_position: usize,
}

impl ErrorDialog {
    fn new(title: &str, message: &str) -> Self {
        ErrorDialog {
            title: title.to_string(),
            message: message.to_string(),
            visible: true,
        }
    }

    fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl EventDetailsDialog {
    fn new(title: &str, content: &str) -> Self {
        EventDetailsDialog {
            title: title.to_string(),
            content: content.to_string(),
            visible: true,
            scroll_position: 0,
        }
    }

    fn dismiss(&mut self) {
        self.visible = false;
    }

    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    fn scroll_down(&mut self, max_lines: usize) {
        let content_lines = self.content.lines().count();
        if content_lines > max_lines && self.scroll_position < content_lines - max_lines {
            self.scroll_position += 1;
        }
    }

    fn page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(10);
    }

    fn page_down(&mut self, max_lines: usize) {
        let content_lines = self.content.lines().count();
        if content_lines > max_lines {
            self.scroll_position = (self.scroll_position + 10).min(content_lines - max_lines);
        }
    }
}

// Struct for application state
struct AppState {
    focus: PanelFocus,
    selected_log_index: usize,
    selected_log_name: String, // Name of the log whose events are shown
    events: Vec<DisplayEvent>,
    table_state: TableState, // Holds scroll/selection state for the event table
    error_dialog: Option<ErrorDialog>, // Optional error dialog
    event_details_dialog: Option<EventDetailsDialog>, // Optional event details dialog
    log_file: Option<std::fs::File>, // Optional log file for debug messages
}

impl AppState {
    fn new() -> Self {
        // Try to open log file
        let log_file = match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("event_commander.log")
        {
            Ok(file) => Some(file),
            Err(_) => None,
        };

        AppState {
            focus: PanelFocus::Logs,
            selected_log_index: 0,
            selected_log_name: "".to_string(),
            events: Vec::new(),
            table_state: TableState::default(),
            error_dialog: None,
            event_details_dialog: None,
            log_file,
        }
    }

    // Log a message to the log file
    fn log(&mut self, message: &str) {
        if let Some(file) = &mut self.log_file {
            use std::io::Write;
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let log_entry = format!("[{}] {}\n", timestamp, message);
            let _ = file.write_all(log_entry.as_bytes());
            let _ = file.flush();
        }
    }

    // Show an error dialog
    fn show_error(&mut self, title: &str, message: &str) {
        self.error_dialog = Some(ErrorDialog::new(title, message));
        // Also log errors
        self.log(&format!("ERROR - {}: {}", title, message));
    }

    // Show event details dialog
    fn show_event_details(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(event) = self.events.get(selected) {
                let title = format!("Event Details: {} ({})", event.source, event.id);
                self.event_details_dialog = Some(EventDetailsDialog::new(&title, &event.raw_data));
                self.log(&format!("Showing details for event ID {}", event.id));
            }
        }
    }

    // Load events using Windows API (Corrected Result handling)
    #[cfg(target_os = "windows")]
    fn load_events_for_selected_log(&mut self) {
        // Get the selected log name
        self.selected_log_name = LOG_NAMES
            .get(self.selected_log_index)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "".to_string());

        // Clear previous events and reset scroll state
        self.events.clear();
        self.table_state = TableState::default();

        if self.selected_log_name.is_empty() {
            self.show_error("Loading Error", "No log name selected.");
            return; // Nothing to load
        }

        // Log that we're starting to load events
        self.log(&format!("Loading events from {}", self.selected_log_name));

        let log_name_wide = to_wide_string(&self.selected_log_name);
        let mut event_log_handle: HANDLE = HANDLE::default();

        // Try with NULL first for local machine
        self.log(&format!("Attempting to open event log: {}", self.selected_log_name));
        
        unsafe {
            // First try NULL for the server name, which is documented to use local computer
            let result = OpenEventLogW(
                PCWSTR::null(),
                PCWSTR::from_raw(log_name_wide.as_ptr())
            );
            
            if let Err(err) = result {
                let error_code = GetLastError();
                self.log(&format!("Failed to open log with NULL server: {:?}, error code: {:?}", 
                         err, error_code));
                
                // Try with "localhost" as server name
                let server_name = to_wide_string("localhost");
                match OpenEventLogW(
                    PCWSTR::from_raw(server_name.as_ptr()),
                    PCWSTR::from_raw(log_name_wide.as_ptr())
                ) {
                    Ok(handle) => {
                        if handle.is_invalid() {
                            self.show_error("Loading Error", 
                                &format!("Invalid handle for log '{}'", self.selected_log_name));
                            return;
                        }
                        event_log_handle = handle;
                        self.log(&format!("Successfully opened event log with 'localhost': {}", 
                                 self.selected_log_name));
                    }
                    Err(error) => {
                        self.show_error("Loading Error", 
                            &format!("Failed to open log '{}': {:?}", self.selected_log_name, error));
                        return;
                    }
                }
            } else {
                event_log_handle = result.unwrap();
                self.log(&format!("Successfully opened event log with NULL server: {}", 
                         self.selected_log_name));
            }
        }

        // --- Read Events ---
        let mut buffer_size: u32 = 8192;
        let mut buffer: Vec<u8> = vec![0; buffer_size as usize];
        let mut bytes_read: u32 = 0;
        let mut bytes_needed: u32 = 0;
        
        // Use sequential and backwards reading flags to get newest events first
        let flags = READ_EVENT_LOG_READ_FLAGS(EVENTLOG_SEQUENTIAL_READ.0 | EVENTLOG_BACKWARDS_READ);
        
        self.log(&format!("Attempting to read events from: {}", self.selected_log_name));
        
        loop {
            if self.events.len() >= EVENT_BATCH_SIZE {
                break; // Stop reading once we have enough events
            }

            let mut success = false;
            unsafe {
                // Try with direct primitive values to avoid any type conversion issues
                let result = ReadEventLogW(
                    event_log_handle,
                    flags,
                    0, // RecordOffset
                    buffer.as_mut_ptr() as *mut core::ffi::c_void,
                    buffer_size,
                    &mut bytes_read,
                    &mut bytes_needed
                );
                
                // Detailed error reporting
                if let Err(ref err) = result {
                    let win_error = GetLastError();
                    self.log(&format!("ReadEventLogW error: {:?}, Win32Error: {:?}, bytes_needed: {}", 
                              err, win_error, bytes_needed));
                    
                    // If buffer is too small, resize and retry
                    if win_error == ERROR_INSUFFICIENT_BUFFER && bytes_needed > 0 {
                        self.log(&format!("Resizing buffer from {} to {} bytes", buffer_size, bytes_needed));
                        buffer.resize(bytes_needed as usize, 0);
                        buffer_size = bytes_needed;
                        continue;
                    }
                }
                
                success = result.is_ok();
            }

            if !success {
                let error = unsafe { GetLastError() };
                match error {
                    ERROR_HANDLE_EOF => {
                        self.log("Reached end of event log");
                        break;
                    }
                    ERROR_INSUFFICIENT_BUFFER => {
                        self.log(&format!("Buffer too small, bytes needed: {}", bytes_needed));
                        buffer.resize(bytes_needed as usize, 0);
                        buffer_size = bytes_needed;
                        continue; // Retry reading
                    }
                    _ => {
                        self.show_error("Reading Error", 
                            &format!("Error reading event log '{}': WIN32_ERROR({}) ", 
                                 self.selected_log_name, error.0));
                        break; // Stop reading on other errors
                    }
                }
            }

            if bytes_read == 0 {
                self.log("No bytes read from log");
                break;
            }

            self.log(&format!("Successfully read {} bytes from event log", bytes_read));

            // --- Process Records in the Buffer ---
            let mut offset: usize = 0;
            
            while offset < bytes_read as usize {
                if self.events.len() >= EVENT_BATCH_SIZE {
                    break;
                }

                // Process the record and log attempts for better debugging
                unsafe {
                    if offset + std::mem::size_of::<EVENTLOGRECORD>() > buffer.len() {
                        self.log(&format!("Buffer too small at offset {} (buffer size: {})", 
                                          offset, buffer.len()));
                        break;
                    }
                    
                    // Access the record directly as an EVENTLOGRECORD
                    let record_ptr = buffer.as_ptr().add(offset) as *const EVENTLOGRECORD;
                    let record = &*record_ptr;
                    
                    // Validate record length
                    if record.Length == 0 {
                        self.log("Found record with zero length, skipping");
                        break;
                    }
                    
                    if offset + record.Length as usize > buffer.len() {
                        self.log(&format!("Record length {} exceeds buffer at offset {}", 
                                         record.Length, offset));
                        break;
                    }
                    
                    self.log(&format!("Processing record: type={:?}, id={}, size={}", 
                                      record.EventType, record.EventID, record.Length));
                    
                    // Extract source name - pass the record pointer directly
                    let source_name = extract_source_name(&buffer, offset, record_ptr);
                    
                    // Log source name for debugging
                    self.log(&format!("Extracted source name: {}", source_name));
                    
                    // *** Event Type Mapping ***
                    // Map the event type to a descriptive string
                    let level = match record.EventType.0 {
                        1 => "Error".to_string(),        // EVENTLOG_ERROR_TYPE = 1u16
                        2 => "Warning".to_string(),      // EVENTLOG_WARNING_TYPE = 2u16
                        4 => "Information".to_string(),  // EVENTLOG_INFORMATION_TYPE = 4u16
                        8 => "Audit Success".to_string(),// EVENTLOG_AUDIT_SUCCESS = 8u16
                        16 => "Audit Failure".to_string(),// EVENTLOG_AUDIT_FAILURE = 16u16
                        other => format!("Unknown({})", other),
                    };
                    
                    // Convert time to readable format
                    let datetime = Utc
                        .timestamp_opt(record.TimeGenerated as i64, 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                        .with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string();
                        
                    // Event ID processing - fix to get both full ID and customer event code
                    let full_event_id = record.EventID;
                    
                    // For display, use the event code (lower 16 bits)
                    let event_code = full_event_id & 0xFFFF;
                    
                    // Capture raw data for display in dialog
                    let mut raw_data = format!("<Event>\n  <System>\n    <Provider Name=\"{}\" />\n    <EventID>{}</EventID>\n    <Level>{}</Level>\n    <Record>{}</Record>\n    <Computer>{}</Computer>\n    <TimeCreated SystemTime=\"{}\"/>\n    <TimeWritten SystemTime=\"{}\"/>\n    <Channel>{}</Channel>\n  </System>\n",
                        source_name,
                        event_code,
                        level,
                        record.RecordNumber,
                        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown".to_string()),
                        datetime,
                        Utc.timestamp_opt(record.TimeWritten as i64, 0).single()
                           .unwrap_or_else(Utc::now).with_timezone(&Local)
                           .format("%Y-%m-%d %H:%M:%S").to_string(),
                        record.EventCategory
                    );
                    
                    // Try to extract event description strings
                    if record.NumStrings > 0 && record.StringOffset > 0 {
                        raw_data.push_str("  <EventData>\n");
                        
                        // Start at the first string (skip the source name)
                        let mut string_pos = offset + record.StringOffset as usize;
                        let mut string_processed = 0;
                        
                        // Skip the first string (source name) by finding its null terminator
                        let source_string_ptr = buffer.as_ptr().add(string_pos) as *const u16;
                        let mut i = 0;
                        
                        unsafe {
                            // Skip the source name string to get to the first actual data string
                            loop {
                                if string_pos + (i * 2) >= buffer.len() {
                                    break;
                                }
                                
                                let c = *source_string_ptr.add(i);
                                if c == 0 {
                                    // Move past the null terminator
                                    string_pos += (i + 1) * 2;
                                    break;
                                }
                                
                                i += 1;
                                if i > 255 { // Safety limit
                                    string_pos += (i + 1) * 2; // Just move ahead
                                    break;
                                }
                            }
                            
                            // Now process the actual message strings (if NumStrings > 1)
                            if record.NumStrings > 1 {
                                self.log(&format!("Processing {} message strings", record.NumStrings - 1));
                                for i in 0..record.NumStrings - 1 { // -1 because we skip source name
                                    if string_pos >= buffer.len() || string_processed >= record.NumStrings - 1 {
                                        break;
                                    }
                                    
                                    // Read the string at current position
                                    let mut string_value = String::new();
                                    let string_ptr = buffer.as_ptr().add(string_pos) as *const u16;
                                    let mut j = 0;
                                    
                                    loop {
                                        if string_pos + (j * 2) >= buffer.len() {
                                            break;
                                        }
                                        
                                        let c = *string_ptr.add(j);
                                        if c == 0 {
                                            break;
                                        }
                                        
                                        if let Some(ch) = char::from_u32(c as u32) {
                                            string_value.push(ch);
                                        }
                                        
                                        j += 1;
                                        if j > 1000 { // Safety limit
                                            string_value.push_str("... (truncated)");
                                            break;
                                        }
                                    }
                                    
                                    self.log(&format!("  Data string {}: {}", i+1, string_value));
                                    raw_data.push_str(&format!("    <Data>{}</Data>\n", string_value));
                                    string_processed += 1;
                                    
                                    // Move to next string (skip null terminator)
                                    string_pos += (j + 1) * 2;
                                }
                            }
                        }
                    }
                    
                    raw_data.push_str("  </EventData>\n");
                    
                    // Try to extract binary data if present
                    if record.DataLength > 0 && record.DataOffset > 0 {
                        let data_start = offset + record.DataOffset as usize;
                        let data_end = (data_start + record.DataLength as usize).min(buffer.len());
                        
                        if data_start < buffer.len() {
                            raw_data.push_str("\nBinary Data (Hex):\n");
                            
                            // Format data as hex dump
                            let data_slice = &buffer[data_start..data_end];
                            for (i, chunk) in data_slice.chunks(16).enumerate() {
                                let hex_line: Vec<String> = chunk.iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect();
                                
                                raw_data.push_str(&format!("  {:04X}: {}\n", i * 16, hex_line.join(" ")));
                            }
                        }
                    }
                    
                    // Close Event tag
                    raw_data.push_str("</Event>");
                    
                    // Add event to our list
                    self.events.push(DisplayEvent {
                        level,
                        datetime,
                        source: source_name,
                        id: event_code.to_string(),
                        raw_data,
                    });
                    
                    // Move to next record
                    offset += record.Length as usize;
                }
            }
            
            if self.events.is_empty() {
                self.log("Finished processing buffer but no valid events found");
            } else {
                self.log(&format!("Processed buffer and found {} events", self.events.len()));
            }
            
            // If we successfully processed some events, we can break the loop
            if !self.events.is_empty() {
                break;
            }
        }

        // --- Close the Handle ---
        unsafe {
            if let Err(error) = CloseEventLog(event_log_handle) {
                self.log(&format!("Error closing event log handle: {:?}", error));
            }
        }

        // Select the first event if any were loaded
        if !self.events.is_empty() {
            self.table_state.select(Some(0));
            self.log(&format!("Loaded {} events from {}", self.events.len(), self.selected_log_name));
        } else {
            self.table_state.select(None);
            self.show_error("Loading Error", &format!("No events found in {}", self.selected_log_name));
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn load_events_for_selected_log(&mut self) {
        // Stub implementation for non-Windows platforms
        self.events.clear();
        self.table_state = TableState::default();
        
        // Add a dummy event for testing
        self.events.push(DisplayEvent {
            level: "Information".to_string(),
            datetime: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            source: "Stub Implementation".to_string(),
            id: "0".to_string(),
            raw_data: String::new(), // Initialize raw_data
        });
        
        if !self.events.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    fn next_log(&mut self) {
        if self.selected_log_index < LOG_NAMES.len() - 1 {
            self.selected_log_index += 1;
        }
    }

    fn previous_log(&mut self) {
        self.selected_log_index = self.selected_log_index.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if self.events.is_empty() {
             self.table_state.select(None);
             return;
         }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.events.len().saturating_sub(1) {
                    i // Stay at bottom
                      // TODO: Add logic here to load more events later
                } else {
                    i + 1
                }
            }
            None => 0, // Select first if nothing is selected
        };
        self.table_state.select(Some(i));
    }

    fn scroll_up(&mut self) {
         if self.events.is_empty() {
             self.table_state.select(None);
             return;
         }
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1), // Safely subtract 1, stays at 0 if already 0
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    // TODO: Make page size dynamic based on visible height
    fn page_down(&mut self) {
        if self.events.is_empty() {
            self.table_state.select(None);
            return;
        }
        let page_size = 10; // Approximate page size
        let i = match self.table_state.selected() {
            Some(i) => (i + page_size).min(self.events.len().saturating_sub(1)),
            None => 0,
        };
         self.table_state.select(Some(i));
          // TODO: Add logic here to load more events later if hitting bottom
    }

    fn page_up(&mut self) {
         if self.events.is_empty() {
             self.table_state.select(None);
             return;
         }
        let page_size = 10; // Approximate page size
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(page_size),
             None => 0,
        };
         self.table_state.select(Some(i));
    }

    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Logs => PanelFocus::Events,
            PanelFocus::Events => PanelFocus::Logs,
        };
    }

    // Make sure we properly handle event record data
    unsafe fn process_event_record(buffer: &[u8], offset: usize) -> Option<(EventLogRecord, String)> {
        if offset + std::mem::size_of::<EventLogRecord>() > buffer.len() {
            return None; // Not enough data for a record
        }
        
        // Read the record header - use an unsafe block
        let record_ptr = unsafe { buffer.as_ptr().add(offset) as *const EventLogRecord };
        let record = unsafe { *record_ptr };
        
        // Validate record
        if record.length == 0 || offset + record.length as usize > buffer.len() {
            return None;
        }
        
        // Get source name from StringOffset - use an unsafe block
        let string_ptr = unsafe { buffer.as_ptr().add(offset + record.string_offset as usize) as *const u16 };
        let mut source_name = String::new();
        
        // Read until null terminator - use an unsafe block
        let mut i = 0;
        loop {
            let c = unsafe { *string_ptr.add(i) };
            if c == 0 {
                break;
            }
            if let Some(ch) = char::from_u32(c as u32) {
                source_name.push(ch);
            }
            i += 1;
            
            // Safety check
            if i > 1000 {
                source_name = String::from("<invalid source>");
                break;
            }
        }
        
        Some((record, source_name))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    let mut terminal = init_terminal()?;

    // Application state
    let mut app_state = AppState::new();

    // Main loop
    loop {
        // Draw UI
        terminal.draw(|frame| ui(frame, &mut app_state))?;

        // Event handling
        if event::poll(Duration::from_millis(100))? { // Reduced poll duration slightly
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_press(key, &mut app_state);
                    if key.code == KeyCode::Char('q') {
                        break; // Check for quit after handling
                    }
                }
            }
        }
    }

    // Restore terminal
    restore_terminal()?;
    Ok(())
}

// Initialize the terminal backend
fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

// Restore terminal state
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

// Render the UI
fn ui(frame: &mut Frame, app_state: &mut AppState) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Left panel for logs
            Constraint::Percentage(70), // Right panel for events
        ])
        .split(frame.size());

    // --- Left Panel (Logs) ---
    let log_items: Vec<ListItem> = LOG_NAMES
        .iter()
        .map(|&name| ListItem::new(name))
        .collect();

    let log_list_block = Block::default()
        .title("Windows Logs")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app_state.focus == PanelFocus::Logs {
            Color::Cyan // Brighter color for focus
        } else {
            Color::White
        }));

    let log_list = List::new(log_items)
        .block(log_list_block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(if app_state.focus == PanelFocus::Logs {
                    Color::Blue
                } else {
                    Color::DarkGray // Dimmer highlight when not focused
                }),
        )
        .highlight_symbol("> ");

    let mut log_list_state = ListState::default();
    log_list_state.select(Some(app_state.selected_log_index));

    frame.render_stateful_widget(log_list, main_layout[0], &mut log_list_state);

    // --- Right Panel (Events Table) ---
    let event_rows: Vec<Row> = app_state
        .events
        .iter()
        .map(|event| {
            Row::new(vec![
                Cell::from(event.level.clone()),
                Cell::from(event.datetime.clone()),
                Cell::from(event.source.clone()),
                Cell::from(event.id.clone()),
            ])
            // You could add per-row styling here based on level, etc.
        })
        .collect();

    let header_cells = ["Level", "Date and Time", "Source", "Event ID"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow) // Header text color
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray)) // Header background
        .height(1)
        .bottom_margin(0); // No margin below header

    // Define column widths - adjust as needed
    let widths = [
        Constraint::Length(10),        // Level width
        Constraint::Length(20),        // DateTime width
        Constraint::Percentage(60),    // Source width (flexible)
        Constraint::Length(10),        // Event ID width
    ];

    let event_table_block = Block::default()
        .title(format!("Events: {}", app_state.selected_log_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app_state.focus == PanelFocus::Events {
            Color::Cyan // Brighter color for focus
        } else {
            Color::White
        }));

    let event_table = Table::new(event_rows, widths)
        .header(header)
        .block(event_table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED)) // Standard row highlight
        .highlight_symbol(">> ") // Symbol for the selected row
        .column_spacing(1); // Spacing between columns


    // Render the table using the state from app_state
    // Crucially, render_stateful_widget needs a mutable reference to the state
    frame.render_stateful_widget(event_table, main_layout[1], &mut app_state.table_state);

    // If there's an event details dialog to display, render it on top
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            // Calculate the dialog size and position (centered, slightly larger than error dialog)
            let dialog_width = 70.min(frame.size().width.saturating_sub(4));
            let dialog_height = 20.min(frame.size().height.saturating_sub(4));
            
            let dialog_area = Rect::new(
                (frame.size().width - dialog_width) / 2,
                (frame.size().height - dialog_height) / 2,
                dialog_width,
                dialog_height,
            );
            
            // Create a clear background for the dialog
            frame.render_widget(Clear, dialog_area);
            
            // Create dialog with blue borders for details
            let dialog_block = Block::default()
                .title(event_details.title.clone())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue));
            
            // Add scrollable content and buttons
            let dialog_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),       // Content area
                    Constraint::Length(3),    // Button area
                ])
                .margin(1)
                .split(dialog_area);
            
            // Render dialog block
            frame.render_widget(dialog_block, dialog_area);
            
            // Render content with scrolling
            let content_lines: Vec<&str> = event_details.content.lines().collect();
            let visible_height = dialog_layout[0].height as usize;
            let start_line = event_details.scroll_position.min(content_lines.len().saturating_sub(1));
            let end_line = (start_line + visible_height).min(content_lines.len());
            
            let visible_content = content_lines[start_line..end_line].join("\n");
            let scroll_info = if content_lines.len() > visible_height {
                format!("[{}/{}]", start_line + 1, content_lines.len())
            } else {
                "".to_string()
            };
            
            let content_paragraph = Paragraph::new(visible_content)
                .wrap(ratatui::widgets::Wrap { trim: false })
                .style(Style::default().fg(Color::White));
            frame.render_widget(content_paragraph, dialog_layout[0]);
            
            // Render scroll position indicator in top-right
            if !scroll_info.is_empty() {
                let scroll_rect = Rect::new(
                    dialog_area.right() - scroll_info.len() as u16 - 2,
                    dialog_area.y + 1,
                    scroll_info.len() as u16,
                    1,
                );
                let scroll_indicator = Paragraph::new(scroll_info)
                    .style(Style::default().fg(Color::Blue));
                frame.render_widget(scroll_indicator, scroll_rect);
            }
            
            // Render dismiss button
            let dismiss_button = Paragraph::new("  [Dismiss (Esc)]  ")
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(Color::White));
            
            // Center the button
            let button_width = 20;
            let button_x = dialog_layout[1].x + (dialog_layout[1].width - button_width) / 2;
            let button_area = Rect::new(
                button_x,
                dialog_layout[1].y,
                button_width,
                3,
            );
            
            frame.render_widget(dismiss_button, button_area);
        }
    }

    // If there's an error dialog to display, render it on top of everything
    if let Some(error_dialog) = &app_state.error_dialog {
        if error_dialog.visible {
            // Calculate the dialog size and position (centered on screen)
            let dialog_width = 60.min(frame.size().width - 4);
            let dialog_height = 10.min(frame.size().height - 4);
            
            let dialog_area = Rect::new(
                (frame.size().width - dialog_width) / 2,
                (frame.size().height - dialog_height) / 2,
                dialog_width,
                dialog_height,
            );
            
            // Create a clear background for the dialog
            frame.render_widget(Clear, dialog_area);
            
            // Create dialog with red borders for errors
            let dialog_block = Block::default()
                .title(error_dialog.title.clone())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red));
            
            // Add message text and dismiss button
            let dialog_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),       // Message area
                    Constraint::Length(3),    // Button area
                ])
                .margin(1)
                .split(dialog_area);
            
            // Render dialog block
            frame.render_widget(dialog_block, dialog_area);
            
            // Render message text
            let message_paragraph = Paragraph::new(error_dialog.message.clone())
                .wrap(ratatui::widgets::Wrap { trim: true })
                .style(Style::default().fg(Color::White));
            frame.render_widget(message_paragraph, dialog_layout[0]);
            
            // Render dismiss button
            let dismiss_button = Paragraph::new("  [Dismiss (Enter)]  ")
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)))
                .style(Style::default().fg(Color::White));
            
            // Center the button
            let button_width = 20;
            let button_x = dialog_layout[1].x + (dialog_layout[1].width - button_width) / 2;
            let button_area = Rect::new(
                button_x,
                dialog_layout[1].y,
                button_width,
                3,
            );
            
            frame.render_widget(dismiss_button, button_area);
        }
    }
}

// Input handling
fn handle_key_press(key: event::KeyEvent, app_state: &mut AppState) {
    // First check if we need to dismiss an error dialog
    if let Some(error_dialog) = &mut app_state.error_dialog {
        if error_dialog.visible {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                error_dialog.dismiss();
                app_state.log("Dismissed error dialog");
            }
            return; // Don't process other keys when dialog is visible
        }
    }
    
    // Check if event details dialog is visible and handle its keys
    if let Some(event_details) = &mut app_state.event_details_dialog {
        if event_details.visible {
            match key.code {
                KeyCode::Esc => {
                    event_details.dismiss();
                    app_state.log("Dismissed event details dialog");
                }
                KeyCode::Up => event_details.scroll_up(),
                KeyCode::Down => event_details.scroll_down(18), // Approximate visible height
                KeyCode::PageUp => event_details.page_up(),
                KeyCode::PageDown => event_details.page_down(18), // Approximate visible height
                _ => {}
            }
            return; // Don't process other keys when dialog is visible
        }
    }

    match app_state.focus {
        // --- LOGS PANEL FOCUS ---
        PanelFocus::Logs => match key.code {
            KeyCode::Char('q') => return, // Let main loop handle quit
            KeyCode::Up => app_state.previous_log(),
            KeyCode::Down => app_state.next_log(),
            KeyCode::Right | KeyCode::Tab => {
                // Load events if switching to a different log or if events are empty
                let current_log_name = LOG_NAMES.get(app_state.selected_log_index).map(|s| s.to_string()).unwrap_or_default();
                if app_state.events.is_empty() || current_log_name != app_state.selected_log_name {
                    app_state.load_events_for_selected_log();
                }
                app_state.switch_focus();
            }
            KeyCode::Enter => {
                app_state.load_events_for_selected_log();
                app_state.switch_focus();
            }
            _ => {}
        },

        // --- EVENTS PANEL FOCUS ---
        PanelFocus::Events => match key.code {
            KeyCode::Char('q') => return, // Let main loop handle quit
            KeyCode::Up => app_state.scroll_up(),
            KeyCode::Down => app_state.scroll_down(),
            KeyCode::PageUp => app_state.page_up(),
            KeyCode::PageDown => app_state.page_down(),
            KeyCode::Enter => app_state.show_event_details(), // Show details for the selected event
            // Handle Left Arrow and Shift+Tab (BackTab)
            KeyCode::Left => app_state.switch_focus(),
            KeyCode::BackTab => app_state.switch_focus(), // BackTab is often Shift+Tab
             // Handle Tab - cycle focus
            KeyCode::Tab => app_state.switch_focus(),
            _ => {}
        },
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct EventLogRecord {
    length: u32,
    reserved: u32,
    record_number: u32,
    time_generated: u32,
    time_written: u32,
    event_id: u32,
    event_type: u16,
    num_strings: u16,
    event_category: u16,
    reserved_flags: u16,
    closing_record_number: u32,
    string_offset: u32,
    user_sid_length: u32,
    user_sid_offset: u32,
    data_length: u32,
    data_offset: u32,
}

#[cfg(target_os = "windows")]
fn extract_source_name(buffer: &[u8], offset: usize, record_ptr: *const EVENTLOGRECORD) -> String {
    unsafe {
        let record = &*record_ptr;
        
        // Remove debug println that interferes with UI
        // println!("Record StringOffset: {}, NumStrings: {}", record.StringOffset, record.NumStrings);
        
        // If no valid offset or no strings, return unknown
        if record.StringOffset == 0 || record.NumStrings == 0 || offset + record.StringOffset as usize >= buffer.len() {
            return "<unknown source>".to_string();
        }
        
        // The source name is the first string after the fixed part of the record,
        // starting at StringOffset which is relative to the start of the record
        let string_ptr = buffer.as_ptr().add(offset + record.StringOffset as usize) as *const u16;
        
        // Read the string (source name) safely
        let mut source = String::new();
        let mut i = 0;
        
        loop {
            // Check boundary
            if offset + record.StringOffset as usize + (i * 2) >= buffer.len() {
                break;
            }
            
            let c = *string_ptr.add(i);
            if c == 0 {
                break; // End of string
            }
            
            if let Some(ch) = char::from_u32(c as u32) {
                source.push(ch);
            }
            
            i += 1;
            if i > 255 { // Safety limit
                break;
            }
        }
        
        // Clean up the source name
        if source.is_empty() {
            return "<unknown source>".to_string();
        }
        
        // If something seems wrong with the source (too long, contains brackets, etc.)
        // it might not be a real source name
        if source.len() > 50 || source.contains('[') || source.contains(']') {
            // Remove debug println
            // println!("Suspicious source name detected: {}", source);
            return "Unknown Provider".to_string();
        }
        
        // Apply known provider name mappings for display consistency
        match source.as_str() {
            s if s.eq_ignore_ascii_case("service control manager") => "Service Control Manager",
            s if s.eq_ignore_ascii_case("microsoft-windows-security-spp") => "Security-SPP",
            s if s.eq_ignore_ascii_case("microsoft-windows-bits-client") => "BITS",
            s if s.contains("SPP") => "Security-SPP",
            s if s.contains("BITS") => "BITS",
            s if s.eq_ignore_ascii_case("perflib") => "Perflib",
            s if s.eq_ignore_ascii_case("BrowserBroker") => "Browser",
            s if s.eq_ignore_ascii_case("eventlog") => "EventLog",
            s if s.eq_ignore_ascii_case("microsoft-windows-wer") 
               || s.eq_ignore_ascii_case("windows error reporting") => "Windows Error Reporting",
            // If the source is all numeric, it's not the real source name
            s if s.chars().all(char::is_numeric) => "Unknown Provider",
            _ => &source,
        }.to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_source_name(_buffer: &[u8], _offset: usize, _string_offset: u32) -> String {
    "Stub Implementation".to_string()
}
