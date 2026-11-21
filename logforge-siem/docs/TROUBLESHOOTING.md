# Troubleshooting

- **Database is locked:** Another process is writing to the SQLite file. LogForge-SIEM uses WAL mode with a 5-second busy timeout and retries automatically.
- **No display found:** On headless Linux sessions, LogForge-SIEM skips the GUI and opens the terminal UI.
