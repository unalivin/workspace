# 0006. A per-user background notifier instead of a system service

Status: accepted

## Context

Users want to hear about urgent items while the application is closed. A system service would need administrator rights, a different mechanism per operating system and a larger attack surface.

## Decision

`templateapp notifier run` is the same binary in a loop: it opens the database read-only, counts notifications newer than its last position and hands a short text to the notification helper of the operating system (`notify-send`, `osascript`, PowerShell). Its position is kept in a small state file, so it never writes to the database.

Start at sign-in is opt-in and per user: an XDG autostart entry on Linux, a LaunchAgent on macOS, a Startup folder entry on Windows. Turning it off removes the entry.

The text is a count by default. Titles are added only when the user turns that on, and never record names or notes.

## Consequences

- Nothing runs in the background unless the user turned it on, and nothing needs administrator rights.
- Notifications appear within the notifier interval, not instantly.
- Whether the desktop displays the notification depends on the helper being present and is outside the application.
