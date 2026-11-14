# Configuration

Everything can be configured inside the application: Settings in the dashboard, Settings in the terminal UI, or `certwatch config`. No setting exists only in a file or an environment variable.

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
| `notifier.show_titles` | `false` | `true`, `false` | Add the alert title, which names the host or the certificate, to system notifications; off shows only a count |
| `checks.interval_minutes` | `0` | `0` to `10080` | Minutes between automatic checks of every enabled endpoint while the dashboard, the terminal UI or `certwatch watch` runs; `0` turns automatic checks off |
| `checks.timeout_seconds` | `10` | `1` to `60` | Seconds to wait for one endpoint to answer |
| `checks.probe_protocols` | `false` | `true`, `false` | Also test which of TLS 1.0, 1.1, 1.2 and 1.3 the endpoint accepts; this opens up to three more connections per check |
| `checks.trust_file` | empty | the path of a PEM file, or empty | Root certificates to trust in addition to the roots of the operating system |
| `checks.history_keep` | `200` | `2` to `5000` | Observations kept per endpoint; older ones are removed after a check |
| `alerts.expiry_days` | `30,14,7,1` | up to 12 whole numbers from `1` to `3650`, separated by commas, or empty | Days before expiry at which an alert is raised; a certificate that has expired always raises one |
| `table.endpoints.columns` | `endpoint,status,certificate,expires,days,checked,tls,state` | a comma-separated subset of `endpoint`, `status`, `certificate`, `expires`, `days`, `checked`, `tls`, `state`; `endpoint` is always included | Columns of the endpoint table |
| `table.endpoints.sort` | `endpoint:asc` | `endpoint`, `expires` or `checked`, then `:asc` or `:desc` | Sort order of the endpoint table |

A test compares this table with the application, so it cannot go out of date unnoticed.

The window position is not saved: the window toolkit does not report it. The window opens centered at the saved size.

Packaged builds are compiled without the toolkit's own animations. The reduced motion setting controls the only animation the application adds itself, the fade of the notice bar.

## Changing and resetting

```bash
certwatch config list
certwatch config get appearance.theme
certwatch config set appearance.theme green
certwatch config reset appearance.theme
certwatch config theme list
```

A value is validated before it is saved. An invalid value is refused with the reason and the allowed values, and the stored value stays as it was. A stored value that a newer or older version does not understand is kept and shown, never dropped.

## Overrides

A flag or an environment variable can override a setting for one run. The stored value is not changed, and every interface shows that the value is overridden and by what.

| Override | Example |
|---|---|
| `--set key=value` (repeatable) | `certwatch --set appearance.density=compact` |
| `CERTWATCH_<KEY>`: the key in upper case with dots replaced by underscores | `CERTWATCH_APPEARANCE_DENSITY=compact` |
| `--db <file>` or `CERTWATCH_DB` | use another database file for this run |
| `--ui gui|tui` or `CERTWATCH_UI` | open this interface without asking |
| `CERTWATCH_CONFIG_DIR` | directory of `config.json` |
| `NO_COLOR` | no colors in the terminal UI |

A flag wins over an environment variable, and both win over the stored value. An override with an invalid value is ignored and reported. [.env.example](../.env.example) lists the variables.

## Notification rules

Each event type has a rule with four fields: enabled, minimum severity, quiet hours (a start and an end as `HH:MM`; the range may cross midnight) and a de-duplication window in minutes.

```bash
certwatch notifications rules list
certwatch notifications rules set record.urgent --min-severity critical --quiet-start 22:00 --quiet-end 07:00
certwatch notifications rules reset record.urgent
```

Run `certwatch notifications rules set --help` for the exact flags.

## Launcher

```bash
certwatch config launcher show
certwatch config launcher set tui
certwatch config launcher reset
```
