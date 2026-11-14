# Configuration

Everything can be configured inside the application: Settings in the dashboard, Settings in the terminal UI, or `templateapp config`. No setting exists only in a file or an environment variable.

## Where values are stored

- **Settings and data** live in the SQLite database, in the `settings` table.
- **The database path and the remembered interface** live in `config.json` in the user configuration directory (see [INSTALLATION.md](INSTALLATION.md)). The directory is created with mode `0700` and the file with mode `0600` on Linux and macOS.

The background notifier keeps its position in `notifier-state.json` in the same directory, so it never has to write to the database. Apart from that, the application writes only the files you export and the optional start-at-sign-in entry.

## Settings

| Key | Default | Allowed values | Meaning |
|---|---|---|---|
| `appearance.mode` | `system` | `system`, `light`, `dark` | Light, dark or follow the operating system |
| `appearance.theme` | `slate` | `slate`, `blue`, `green`, `crimson`, `amber`, `purple`, `contrast` | Color theme |
| `appearance.accent` | empty | `#RRGGBB` or empty | Custom accent color; adjusted when needed to keep WCAG AA contrast |
| `appearance.density` | `comfortable` | `comfortable`, `compact` | Row height and spacing |
| `appearance.text_scale` | `100` | `80` to `200` | Text size in percent |
| `appearance.reduced_motion` | `false` | `true`, `false` | Turn off the fade of the notice bar |
| `appearance.monochrome` | `false` | `true`, `false` | Draw the terminal UI without colors |
| `display.time_zone` | `local` | `local`, `utc` | Time zone of displayed timestamps |
| `window.sidebar_collapsed` | `false` | `true`, `false` | Show the sidebar as icons only |
| `window.width` | `1180` | `640` to `10000` | Dashboard window width, saved when the window closes |
| `window.height` | `760` | `480` to `10000` | Dashboard window height, saved when the window closes |
| `data.refresh_seconds` | `2` | `1` to `60` | Seconds between automatic refreshes of the terminal UI |
| `safety.confirm_destructive` | `true` | `true`, `false` | Ask before deleting or overwriting data |
| `notifier.interval_seconds` | `60` | `15` to `3600` | Seconds between checks of the background notifier |
| `notifier.show_titles` | `false` | `true`, `false` | Add the notification title to system notifications |
| `table.records.columns` | `id,name,status,severity,updated,state` | a comma-separated subset of `id`, `name`, `status`, `severity`, `updated`, `state`; `name` is always included | Columns of the record table |
| `table.records.sort` | `updated:desc` | `name`, `status`, `severity` or `updated`, then `:asc` or `:desc` | Sort order of the record table |

A test compares this table with the application, so it cannot go out of date unnoticed.

The window position is not saved: the window toolkit does not report it. The window opens centered at the saved size.

Packaged builds are compiled without the toolkit's own animations. The reduced motion setting controls the only animation the application adds itself, the fade of the notice bar.

## Changing and resetting

```bash
templateapp config list
templateapp config get appearance.theme
templateapp config set appearance.theme green
templateapp config reset appearance.theme
templateapp config theme list
```

A value is validated before it is saved. An invalid value is refused with the reason and the allowed values, and the stored value stays as it was. A stored value that a newer or older version does not understand is kept and shown, never dropped.

## Overrides

A flag or an environment variable can override a setting for one run. The stored value is not changed, and every interface shows that the value is overridden and by what.

| Override | Example |
|---|---|
| `--set key=value` (repeatable) | `templateapp --set appearance.density=compact` |
| `TEMPLATEAPP_<KEY>`: the key in upper case with dots replaced by underscores | `TEMPLATEAPP_APPEARANCE_DENSITY=compact` |
| `--db <file>` or `TEMPLATEAPP_DB` | use another database file for this run |
| `--ui gui|tui` or `TEMPLATEAPP_UI` | open this interface without asking |
| `TEMPLATEAPP_CONFIG_DIR` | directory of `config.json` |
| `NO_COLOR` | no colors in the terminal UI |

A flag wins over an environment variable, and both win over the stored value. An override with an invalid value is ignored and reported. [.env.example](../.env.example) lists the variables.

## Notification rules

Each event type has a rule with four fields: enabled, minimum severity, quiet hours (a start and an end as `HH:MM`; the range may cross midnight) and a de-duplication window in minutes.

```bash
templateapp notifications rules list
templateapp notifications rules set record.urgent --min-severity critical --quiet-start 22:00 --quiet-end 07:00
templateapp notifications rules reset record.urgent
```

Run `templateapp notifications rules set --help` for the exact flags.

## Launcher

```bash
templateapp config launcher show
templateapp config launcher set tui
templateapp config launcher reset
```
