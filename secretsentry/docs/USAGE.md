# SecretSentry usage guide

SecretSentry keeps a list of records in one SQLite file on your computer. It has three interfaces over the same data: a dashboard window, a full-screen terminal UI and commands. This guide is built into the application: open About, Usage guide, or run `secretsentry guide`.

## Getting started

Start the application without a command. It asks how you want to open it:

- **Dashboard (graphical window)** opens the desktop window.
- **Terminal (text interface)** opens the full-screen terminal UI.

Select **Remember my choice** to skip the question next time. Change the answer later in Settings, Open with, or ask again once with `secretsentry --ask`.

On the first start the application creates its database and tells you where the file is. Choose **Load demo data** to look around with sample records. Demo records have names that start with `Demo:` and are removed with one action in Settings.

## Overview

Overview shows what needs attention: open records, urgent records (open with high or critical severity), unread notifications and disabled records, followed by a table of enabled records by status and severity. The dashboard also draws those numbers as a tally: one row per severity, one column per status, one square per record. When a count does not fit, a square stands for more records and the line under the tally says how many. The dashboard, the terminal UI and `secretsentry overview` read the same numbers from the same database views.

## Records

A record has a name, a status (Open, In progress, Done), a severity (Info, Low, Medium, High, Critical), an optional note and an enabled switch. Disabled records are kept but not counted.

- **Add**: Add record in the dashboard, `n` in the terminal UI, `secretsentry records add --name "..."`.
- **Edit**: select a row and choose Edit, `e`, or `secretsentry records edit <id>`.
- **Enable or disable**: the switch in the detail panel, `x`, or `secretsentry records enable|disable <id>`.
- **Remove**: Remove in the detail panel, `d`, or `secretsentry records remove <id>`. The application states what is removed and asks first. The command needs `--yes` when it cannot ask, and `--dry-run` shows what would happen.
- **Search**: type in the search box, press `/` in the terminal UI, or add `--search` to `records list`. Search covers names and notes.
- **Filter and sort**: by status and severity, with disabled records hidden until you show them. The sort column and direction are remembered and are the same in the dashboard and the terminal UI.
- **Columns**: choose Columns in the dashboard or press `c` in the terminal UI to show or hide the ID, Status, Severity, Updated and State columns. The name is always shown. The choice is remembered.
- **Copy**: Copy in the detail panel of the dashboard puts the record on the clipboard as text.

If two windows edit the same record, the second save is refused with an explanation and your text stays in the form. Nothing is overwritten silently.

## Import and export

Export writes every record to CSV or JSON. The file is readable only by your user account. Import reads CSV or JSON, matches rows by name, adds new names and updates known ones. The dashboard and the terminal UI always show a dry run first: how many rows would be created, updated, left unchanged and rejected, with the reason for each rejected row. Nothing is written until you confirm. Files larger than 5 MB or 10000 rows are refused.

CSV columns: `name`, `status`, `severity`, `note`, `enabled`. Only `name` is required.

## Notifications

The application raises a notification when an open record has high or critical severity and when a record is completed. Notifications appear in the Notifications category with a severity icon and label, a time and a link to the record.

Each event type has a rule: enabled or not, a minimum severity, optional quiet hours and a de-duplication window in minutes. Change rules in Notifications, Rules or with `secretsentry notifications rules set`.

### System notifications while the application is closed

This is off until you turn it on. Settings, Notifications, Start at sign-in adds a per-user entry that runs `secretsentry notifier run` when you sign in. It needs no administrator rights, opens the database read-only and makes no network requests. By default a system notification shows only a count. Turn on Show titles in system notifications to add the notification title. Record names and notes are never shown.

## Settings

Every setting can be changed in the dashboard, in the terminal UI and with `secretsentry config set <key> <value>`. Each setting shows where its value comes from: the default, the stored value, an environment variable or a command-line flag. A value that is overridden says by what. Invalid values are refused with the reason and an example. Reset returns one setting to its default.

### Appearance

Choose Light, Dark or Follow system and one of seven color themes: Neutral slate, Blue, Green, Crimson, Amber, Purple and High contrast. A custom accent color is checked against the background and adjusted when it would be hard to read. Severity colors never depend on the theme accent and always come with an icon and a label. Density, text size and reduced motion apply at once.

The terminal UI uses the same themes, works with 16 colors, honors `NO_COLOR` and has a monochrome setting.

### Database location

Settings, Database location shows the file in use. Choose a new location to copy your data there and use it from then on, or switch to a database that already exists. The old file is never deleted. When the location is set by `--db` or by the environment variable, the setting says so and cannot be changed until the override is removed.

## Keyboard

Every action is reachable from the keyboard. About, Shortcuts lists the keys for the dashboard and the terminal UI. In the terminal UI, `:` opens the menu and `?` opens this guide.

## Commands

`secretsentry --help` lists every command, and `secretsentry <command> --help` explains one. Commands never ask a question when they cannot: scripts and CI get an error that names the flag to add. Exit code 0 means success, 1 means the action failed and 2 means the command line was wrong.

## Privacy

The application makes no network requests. Nothing is collected or sent. Links to the project page open in your browser only when you choose them, and a bug report shows its exact text before the browser opens.

## Troubleshooting

- **The dashboard does not open**: on a computer without a display the application opens the terminal UI and says why. `secretsentry --ui tui` asks for the terminal UI directly.
- **The database is busy**: another window is writing. The application waits, retries and tells you. Nothing is lost.
- **A file is not a database**: the application refuses to open or overwrite it. Choose another location.
- **Check the database**: About, Diagnostics or `secretsentry db check`.
