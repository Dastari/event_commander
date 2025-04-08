## v1.0.2 - Help Screen & Filter Improvements

This release introduces a user help screen and significantly enhances the event filtering experience.

### ✨ New Features & Enhancements

*   **Help Screen (F1):**
    *   Added a comprehensive Help Screen accessible globally by pressing `F1`.
    *   Displays project information, author (Toby Martin), source code link (`https://github.com/Dastari/event_commander`), and license details (GPL-3.0-or-later).
    *   Includes a detailed list of all application keybindings.
    *   The help screen is scrollable using standard navigation keys (`↑`/`↓`, `PgUp`/`PgDn`, `Home`/`End`).
*   **Improved Source Filtering (Filter Dialog - `f` key):**
    *   The "Source" input field now acts like a searchable dropdown.
    *   Typing filters the list of available event sources in real-time.
    *   Use `↑`/`↓` keys to navigate the filtered list.
    *   Press `Enter` to select a highlighted source and populate the input field.
    *   Pressing `Tab` now correctly moves focus to the next field without selecting the highlighted source from the list.
    *   The dialog now correctly initializes the "Source" input field with the currently active filter value when opened.
*   **Version Display:** The application version (`v1.0.2`) is now displayed in the Help Screen title and subtly in the bottom-right corner of the Event Message Preview panel border.
*   **License:** The project is now explicitly licensed under `GPL-3.0-or-later`, reflected in `LICENSE.md` and `Cargo.toml`.

### 🐛 Bug Fixes

*   Fixed an issue where the first alphabetically sorted source (often ".NET Runtime") would always appear in the source filter list, regardless of the typed filter text.
*   Corrected the Filter Dialog layout calculation to ensure all fields (like "Event ID") remain visible when the source filter list is displayed.
*   Improved keybinding alignment in the `F1` Help Screen for better readability.

### Assets

*(Optional: If you attach the compiled executable)*
*   `event_commander-v1.0.2-windows.zip`: Compiled executable for Windows. 