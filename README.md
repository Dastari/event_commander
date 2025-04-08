# Event Commander

## Overview

Event Commander is a command-line based event viewer for Windows, designed to replicate the core functionality of the Windows Event Viewer within a terminal interface. It aims to provide a user experience reminiscent of classic DOS utilities like Norton Commander or Midnight Commander, utilizing text-based windows, menus, and keyboard navigation.

The entire application will be built using Rust.

## Goals

*   Provide access to Windows Event Logs (Application, System, Security, Custom Logs, etc.) via a TUI.
*   Allow users to view, filter, and search events efficiently from the command line.
*   Offer an intuitive keyboard-driven navigation system.
*   Present event data clearly within an ASCII-based interface.

## Technology Stack

*   **Language:** Rust
*   **Build Tool:** Cargo

## Proposed Libraries & Tools

*   **Text-based User Interface (TUI):**
    *   **`ratatui`**: A modern and actively maintained fork of `tui-rs`. It provides widgets and tools for building complex terminal UIs. It works well with various backends.
    *   **`crossterm`**: Often used as a backend for `ratatui`. Handles terminal manipulation like raw mode, cursor positioning, colors, and input events (keyboard/mouse) across different platforms (though we are primarily targeting Windows).
    *   *Alternatives:* `cursive` offers a higher-level, view-based approach but might be less flexible for a commander-style UI.

*   **Windows Event Log Access:**
    *   **`windows-rs`**: Official Rust bindings for Windows APIs generated by Microsoft. Provides direct access to the necessary Win32 functions for event logging (e.g., within `Win32::System::EventLog`). This offers the most control but requires interacting with lower-level C-style APIs.
    *   **`winevt_rs`**: A higher-level wrapper specifically for the Windows Event Log API, built on top of `windows-rs`. This might simplify development by providing safer abstractions over the raw Win32 functions.

*   **Keyboard Input:**
    *   Handled primarily by the TUI library backend (e.g., `crossterm` when used with `ratatui`). It captures key presses and translates them into events the application can react to.

## Potential Features

*   List available event logs.
*   Display events in a filterable list view (e.g., main panel).
*   Show detailed information for a selected event (e.g., in a separate panel or dialog).
*   Filter events by:
    *   Log Level (Information, Warning, Error, etc.)
    *   Source
    *   Event ID
    *   Date/Time Range
    *   Keywords
*   Text search within event messages.
*   Keyboard navigation (arrow keys, page up/down, home/end, function keys for actions).
*   Configurable display columns.
*   (Future) Event log clearing/backing up.
*   (Future) Watching logs for real-time events.

## Interface Style

The UI should draw inspiration from Norton Commander / Midnight Commander:
*   Use panels to display information (e.g., list of logs, list of events, event details).
*   Employ menus or function key bars for commands.
*   Use text-based dialogs for filtering/searching/options. 