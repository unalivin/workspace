# Roadmap

Template App is at `0.1.0-dev`. This page lists what does not exist yet. Nothing here is promised for a date.

## Not implemented

- Saving and restoring the window position. Only the size and the sidebar state are saved.
- Copying a record to the clipboard from the terminal UI. The dashboard has Copy; in a terminal use the terminal's own selection or `templateapp records show <id>`.
- Resizing table columns with the pointer and reordering columns. Columns can be shown and hidden.
- Hiding the console window on Windows. The executable is a console application, so starting the dashboard by double-click also opens a console window.
- Charts. The Overview shows numbers and a table.
- Signed and notarized downloads, an installer, automatic updates.
- Translations. The interface is in English.
- Encryption of the database file.
- Builds for Linux on ARM and for macOS on Intel processors.

## Not verified

- The dashboard on a real display, with a screen reader, and on high-DPI screens.
- Display of system notifications, start at sign-in at a real sign-in, and opening a terminal window on each operating system.
- Quit from the application menu on macOS (Cmd+Q): whether the window size is saved on that path.
- The terminal UI in a real terminal emulator. The tests drive its model and check the painted screen in memory.

See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md) for what the automated checks cover.

## Next

1. Verify the dashboard by hand on Linux, macOS and Windows and record the result in `PLATFORM_SUPPORT.md`.
2. Hide the console window on Windows when the dashboard is started without a console, and keep command output working.
3. Save the window position on the platforms where the toolkit allows it.
