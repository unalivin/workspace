# Troubleshooting

Start with `certwatch diagnostics` (or About, Diagnostics). It shows the version, the database in use and who set its location, the schema version, an integrity check, and every setting that differs from its default or is overridden.

## Starting

**The dashboard does not open and the terminal UI appears instead.**
The session has no display (a server, SSH, a container). The application says so in one line. On a desktop, run `certwatch --ui gui` to see the reason.

**"this build has no dashboard".**
You are running the no-GUI build. Use the regular download, or the terminal UI and the commands.

**The application no longer asks how to open.**
A choice is remembered. Run `certwatch --ask` once, or change it in Settings, Open with, or with `certwatch config launcher reset`.

**The terminal UI says the terminal is too small.**
Enlarge the window to at least the size it names, or use the commands.

**Colors look wrong in the terminal.**
Set `NO_COLOR=1`, or turn on Monochrome terminal in Settings. The terminal UI works with 16 colors.

**macOS or Windows refuses to start the download.**
The builds are not signed. See "Open an unsigned build" in [INSTALLATION.md](INSTALLATION.md).

**On Windows, command output appears after the prompt.**
The executable is a window application. Append `| more` or redirect the output to a file.

## Database

**"The database is busy".**
Another window is writing. The application waits and retries by itself; nothing is lost. If it never ends, close the other window.

**"is not a usable SQLite database".**
The chosen file exists and is something else. The application never overwrites it. Choose another location.

**"the database has schema version N but this build knows only M".**
The database was written by a newer build. Use that build. The file is not changed.

**A changed location is ignored.**
`--db` or `CERTWATCH_DB` is set and wins over the saved location. Settings, Database location names the override. Remove it and start again.

**Check or repair.**
`certwatch db check` runs SQLite's integrity check and reports problems. To go back to a known state, export the endpoint list (`certwatch endpoints export --file endpoints.csv`), move the database file away and import the list into the new, empty database (`certwatch endpoints import --file endpoints.csv --authorized`). The list carries the endpoints, not their observation history; a check fills the inventory again.

**Move the data to another computer.**
Close the application and copy the database file. That keeps the history. `certwatch endpoints export` and `certwatch endpoints import` move only the endpoint list.

## Checks

**"the host name could not be resolved".**
The name has no address on this computer's resolver. Check the spelling. An internal name needs the internal resolver or a VPN.

**"no answer within N seconds".**
The host is down, filtered by a firewall, or slow. Raise `checks.timeout_seconds` (up to 60) if the host is known to be slow.

**"the connection was refused".**
Nothing listens on that port. Check the port of the endpoint.

**The handshake fails with a protocol error.**
The port does not speak TLS directly. Services that start in clear text and upgrade (STARTTLS on ports such as 25, 143 or 587) are not supported. Monitor the port that starts with TLS, for example 465, 993 or 636.

**The chain of an internal host is "not trusted".**
The root certificate of your certificate authority is not in the operating system's trust store. Set `checks.trust_file` to a PEM file with that root, or install the root in the operating system.

**"the chain does not lead to a trusted root certificate" for a public host.**
Most often the server does not send its intermediate certificate. Browsers sometimes repair this on their own; other clients fail. Configure the server to send the full chain.

**A name mismatch for a host given as an IP address.**
Set the server name of the endpoint to the name the certificate is issued for.

**A check asks for confirmation although confirmations are turned off.**
That is intended. `safety.confirm_destructive` covers removals and overwrites. Network actions always list their connections and ask. Commands need `--yes`.

**"confirm that you are authorized to test this host".**
Adding an endpoint, changing its host or port, and importing a list need the confirmation: the checkbox in the form, or `--authorized`.

**Nothing is checked automatically.**
Automatic checks are off until `checks.interval_minutes` is set, and they run only while the dashboard, the terminal UI or `certwatch watch` is open. Demo endpoints and disabled endpoints are never checked.

**The accepted TLS versions are empty.**
They are recorded only when `checks.probe_protocols` is on or after Probe TLS versions in Configuration.

## Settings

**A setting does not change.**
It is overridden by a flag or an environment variable. The setting shows the override; `certwatch config list` shows it in the SOURCE column. Your change is saved and applies once the override is gone.

**A value is refused.**
The message names the allowed values or the range. Nothing is saved until the value is valid.

## Import

**Rows are rejected.**
The preview lists each rejected row with its line and the reason. Fix the file and try again; nothing was written.

**The file is refused as too large.**
Endpoint lists are limited to 4 MB and 5000 rows, PEM files to 4 MB and 500 certificates. Split the file.

## Alerts and notifications

**No alert for a certificate that expires soon.**
Alerts are raised by a check, at the thresholds in `alerts.expiry_days`, once per certificate and threshold. A disabled endpoint raises none, and a rule in Alerts, Rules can turn an event off, set a minimum severity or quiet hours.

**No system notification appears.**
System notifications are off until you turn on Start at sign-in. Check `certwatch notifier status`. On Linux the notifier uses `notify-send`, which must be installed. Display by the operating system is not verified by the automated tests.

**Too many notifications.**
Raise the minimum severity, set quiet hours or a longer de-duplication window in Alerts, Rules, or shorten the list in `alerts.expiry_days`.

## Reporting a bug

About, "Found a bug? Please report it on GitHub." shows the exact text of the report before your browser opens: version, operating system, architecture, build information and the names of changed settings. It contains no host names, no notes and no paths.
