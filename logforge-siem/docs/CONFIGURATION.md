# Configuration

All settings are stored in the SQLite `settings` table so the desktop GUI, terminal UI, and CLI stay in sync.

- `--db <path>`: SQLite database path (default: `logforge-siem.db`).
- `LOGFORGE_SIEM_UI`: Set to `gui` or `tui` to override the launcher choice.
- `NO_COLOR`: Disables ANSI colors in the terminal UI.
