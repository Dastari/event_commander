# Event Commander

## Overview

Event Commander is a terminal-based event viewer for Windows, written in Rust. It provides a user experience reminiscent of classic file managers like Norton Commander, allowing users to navigate and view Windows Event Logs efficiently using keyboard controls within a TUI (Text-based User Interface).

## Features

- **Log Selection:** Navigate and select from standard Windows Event Logs (Application, System, Security, Setup, ForwardedEvents) in the left panel.
- **Event Listing:** View events from the selected log in a table format (Level, DateTime, Source, Event ID) in the main panel.
- **Dynamic Loading:** Events are fetched in batches as you scroll down the event list.
- **Event Details:** Press Enter on an event to open a detailed view dialog.
- **Multiple Detail Views:** Toggle between a formatted summary and the raw event XML within the details dialog using 'v'.
- **XML Pretty Printing:** The raw XML view is automatically pretty-printed for readability.
- **Save Event:** Save the full, pretty-printed XML of the selected event to a local file using 's' in the details dialog.
- **Preview Pane:** See a preview of the selected event's formatted message at the bottom.
- **Keyboard Navigation:** Use arrow keys, PageUp/Down, Home/End, Tab/BackTab, and Enter/Esc for navigation and interaction.
- **Logging:** Application status and errors are logged to `event_commander.log`.

## Building and Running

1.  **Prerequisites:**
    - Install Rust and Cargo: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
    - Install Windows SDK on your Windows system
    - Install MinGW-w64 toolchain in WSL:
      ```bash
      sudo apt-get update && sudo apt-get install -y mingw-w64 gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
      ```
2.  **Clone the repository:**
    ```bash
    git clone <repository_url>
    cd event-commander
    ```
3.  **Build:**
    ```bash
    cargo build --target x86_64-pc-windows-gnu --release
    ```
    _Note: Building `windows-rs` can take some time, especially the first time._
4.  **Run:**
    ```bash
    cargo run --target x86_64-pc-windows-gnu --release
    ```
    Alternatively, run the compiled executable directly:
    ```bash
    ./target/x86_64-pc-windows-gnu/release/event_commander.exe
    ```
