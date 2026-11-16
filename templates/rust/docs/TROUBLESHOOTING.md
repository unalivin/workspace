# Troubleshooting

Start with `templateapp diagnostics` (or About, Diagnostics). It shows the version, the database in use and who set its location, the schema version, an integrity check, and every setting that differs from its default or is overridden.

## Starting

**The dashboard does not open and the terminal UI appears instead.**
The session has no display (a server, SSH, a container). The application says so in one line. On a desktop, run `templateapp --ui gui` to see the reason.

**"this build has no dashboard".**
You are running the no-GUI build. Use the regular download, or the terminal UI and the commands.

**The application no longer asks how to open.**
A choice is remembered. Run `templateapp --ask` once, or change it in Settings, Open with, or with `templateapp config launcher reset`.

**The terminal UI says the terminal is too small.**
Enlarge the window to at least the size it names, or use the commands.

**Colors look wrong in the terminal.**
Set `NO_COLOR=1`, or turn on Monochrome terminal in Settings. The terminal UI works with 16 colors.

**macOS or Windows refuses to start the download.**
The builds are not signed. See "Open an unsigned build" in [INSTALLATION.md](INSTALLATION.md).

**On Windows, a console window opens next to the dashboard.**
The executable is a console application, so that commands and the terminal UI work in a console. Started by double-click, Windows gives it a console window; closing that window closes the dashboard. Hiding it is on the [roadmap](ROADMAP.md).

## Database

**"The database is busy".**
Another window is writing. The application waits up to five seconds before it reports this; nothing is lost. Try again, and if it never ends, close the other window.

**"is not a usable SQLite database".**
The chosen file exists and is something else. The application never overwrites it. Choose another location.

**"the database has schema version N but this build knows only M".**
The database was written by a newer build. Use that build. The file is not changed.

**A changed location is ignored.**
`--db` or `TEMPLATEAPP_DB` is set and wins over the saved location. Settings, Database location names the override. Remove it and start again.

**Check or repair.**
`templateapp db check` runs SQLite's integrity check and reports problems. To go back to a known state, export your records (`templateapp records export`), move the database file away and import them into the new, empty database.

**Move the data to another computer.**
Close the application and copy the database file, or use `templateapp records export` and `templateapp records import`.

## Settings

**A setting does not change.**
It is overridden by a flag or an environment variable. The setting shows the override; `templateapp config list` shows it in the SOURCE column. Your change is saved and applies once the override is gone.

**A value is refused.**
The message names the allowed values or the range. Nothing is saved until the value is valid.

## Import

**Rows are rejected.**
The dry run lists each rejected row with its line and the reason. Fix the file and try again; nothing was written.

**The file is refused as too large.**
Imports are limited to 5 MB and 10000 rows. Split the file.

## Notifications

**No system notification appears.**
System notifications are off until you turn on Start at sign-in. Check `templateapp notifier status`. On Linux the notifier uses `notify-send`, which must be installed. Display by the operating system is not verified by the automated tests.

**Too many notifications.**
Raise the minimum severity, set quiet hours or a longer de-duplication window in Notifications, Rules.

## Reporting a bug

About, "Found a bug? Please report it on GitHub." shows the exact text of the report before your browser opens: version, operating system, architecture, build information and the names of changed settings. It contains no record content and no paths.
