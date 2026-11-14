# CertWatch usage guide

CertWatch keeps an inventory of TLS certificates in one SQLite file on your computer. It connects to the hosts you list, records the certificate chain each one presents, and tells you what expires soon, what fails validation and what changed. It has three interfaces over the same data: a dashboard window, a full-screen terminal UI and commands. This guide is built into the application: open About, Usage guide, or run `certwatch guide`.

Check only hosts you are authorized to test. CertWatch connects to a host only after you have confirmed that for the host, and it shows every connection before it makes one.

## Getting started

Start the application without a command. It asks how you want to open it:

- **Dashboard (graphical window)** opens the desktop window.
- **Terminal (text interface)** opens the full-screen terminal UI.

Select **Remember my choice** to skip the question next time. Change the answer later in Settings, Open with, or ask again once with `certwatch --ask`.

On the first start the application creates its database and tells you where the file is. Then either add your first endpoint or choose **Load demo data** to look around.

Demo data is 11 endpoints with generated certificates. Their hosts end in `.demo.invalid`, every demo row is labeled `(demo)`, and demo endpoints are never contacted. Remove them with one action: Settings, Data, Remove demo data, or `certwatch demo remove`.

## Overview

Overview answers what needs attention now. Each count opens the list behind it:

- **Expired**, **Expiring in 7 days**, **Expiring in 30 days**, **Expiring in 90 days**: tracked certificates by time left. A certificate that expires in 5 days is counted in all three expiring counts.
- **Chain failures**: endpoints whose chain does not lead to a trusted root.
- **Name mismatches**: endpoints whose certificate is not valid for the name you monitor.
- **Failed checks**: endpoints that did not answer or did not complete a TLS handshake at the last check.
- **Not checked yet**: enabled endpoints without an observation.

Below the counts are the certificates that expire soonest and the most recent alerts. The dashboard, the terminal UI and `certwatch overview` read the same numbers from the same database views. Disabled endpoints are left out of every count.

## Endpoints

An endpoint is a host name or IP address and a port, by default 443. Optional fields are the server name to ask for and to verify when it differs from the host (for example when the host is an IP address) and a note for your own reference.

- **Add**: Add endpoint in the dashboard, `n` in the terminal UI, `certwatch endpoints add --host www.example.org --authorized`. You must confirm that you are authorized to test the host. Without the confirmation nothing is saved.
- **Check now**: select a row and choose Check now, `c`, or `certwatch endpoints check <id>`. **Check all** (`C`, `certwatch endpoints check-all`) checks every enabled endpoint.
- **Edit**: Edit, `e`, or `certwatch endpoints edit <id>`. Changing the host or the port needs the authorization confirmation again.
- **Enable or disable**: `x` and `X` in the terminal UI, the buttons in the detail panel, or `certwatch endpoints enable|disable <id>`. A disabled endpoint is kept but is not checked, not counted and raises no alerts.
- **Remove**: Remove, `d`, or `certwatch endpoints remove <id>`. The endpoint and its observation history are removed. The application states what is removed and asks first.
- **Search and filter**: type in the search box, press `/` in the terminal UI, or add `--search` and `--filter` to `endpoints list`. Filters: All, Enabled, Disabled, Not checked yet, Failed checks, Chain failures, Name mismatches, Changed certificate.
- **Columns and sort order**: the settings `table.endpoints.columns` and `table.endpoints.sort` apply to the dashboard, the terminal UI and `endpoints list`.

The detail of an endpoint shows the last result, the findings, the chain as the server sent it, the latest observations and the history of certificate changes.

If two windows edit the same endpoint, the second save is refused with an explanation and your text stays in the form. Nothing is overwritten silently.

## What a check does

A check resolves the host name, opens one TCP connection to the host and port and performs a TLS handshake. It sends no request after the handshake and closes the connection. With the setting `checks.probe_protocols` on, or with Probe TLS versions in Configuration, it opens up to three more connections to see which of TLS 1.0, 1.1, 1.2 and 1.3 the server accepts.

Before any connection the application lists exactly what it will connect to and asks. This question is asked even when confirmations for destructive actions are turned off. Commands need `--yes`, and `--dry-run` prints the connections without making them.

Certificate verification is never turned off. When a chain is not trusted, CertWatch still records the certificates the server presented, marks the chain as not trusted and says why. It never treats an untrusted certificate as valid.

A check records:

- the leaf certificate and the chain: subject, issuer, subject alternative names, validity, key, signature algorithm, SHA-256 fingerprint
- whether the chain leads to a trusted root, and whether the certificate is valid for the monitored name
- the negotiated TLS version and cipher suite, and the accepted versions when probing is on
- findings: expired, not yet valid, self-signed, untrusted chain, name mismatch, weak key, weak signature, validity longer than 398 days, old TLS versions accepted, TLS 1.3 not accepted, weak cipher suite

Trusted roots are those of the operating system. To trust a private certificate authority as well, set `checks.trust_file` to a PEM file with its root certificate.

### Automatic checks

Automatic checks are off by default. Set `checks.interval_minutes` to a number of minutes and every enabled endpoint is checked at that interval while the dashboard, the terminal UI or `certwatch watch` is running. CertWatch installs no service: nothing is checked while none of them runs.

`certwatch watch` runs in a terminal until you interrupt it. `certwatch watch --once --yes` checks every endpoint that is due once and exits, which suits a scheduler you control.

## Certificates

Certificates lists every certificate that an enabled endpoint presents and every certificate you imported from a file. Filters: Tracked, Expiring in 7, 30 or 90 days, Expired, Authorities, All stored.

- **Import PEM file**: `i`, or `certwatch certificates import --file chain.pem`. A preview shows what would be added before anything is written. Private keys in the file are ignored and never stored. Files larger than 4 MB or with more than 500 certificates are refused.
- **Save as PEM**: `o`, or `certwatch certificates export <id> --file out.pem`. The file is readable only by your user account. An existing file is not overwritten unless you say so.
- **Stop tracking**: `d`, or `certwatch certificates stop-tracking <id>`, removes an imported certificate from the inventory. A certificate that an endpoint presents stays tracked for as long as the endpoint presents it.

The detail of a certificate shows its fields, its subject alternative names, its chain, the endpoints that present it and its change history across observations.

## Expiry Timeline

The timeline lists tracked certificates in the order in which they expire, with a bar for the time left. The zoom sets how far ahead it looks: 30, 90 or 180 days, a year, or everything. Change the zoom with the Zoom box in the dashboard, `f` and `F` in the terminal UI, or `certwatch timeline list --zoom 30`.

## Chains

Chains shows for every enabled endpoint whether its chain leads to a trusted root, how long the chain is, who issued the certificate, and the reason when validation failed. Filters: All, Failed validation, Valid.

## Configuration

Configuration shows the TLS version and cipher suite each endpoint negotiated, the versions it accepts when they were probed, and the number and worst severity of its findings. **Probe TLS versions** (`c`, or `certwatch configuration probe <id>`) connects once per TLS version; it is a network action and asks first.

## Alerts

The application raises an alert when:

- a certificate crosses an expiry threshold. The thresholds are the days in the setting `alerts.expiry_days`, by default 30, 14, 7 and 1. Each threshold raises one alert per certificate, and an expired certificate always raises one.
- chain validation or host name validation fails. The alert is raised when the result turns from good to bad, not at every check.
- an endpoint presents a different certificate than at its last successful check.

Alerts appear in the Alerts category with a severity icon and label, a time and a link to the endpoint or the certificate. Mark one as read, or all at once.

Each event type has a rule: enabled or not, a minimum severity, optional quiet hours and a de-duplication window in minutes. Change rules in Alerts, Rules or with `certwatch alerts rules set`.

### System notifications while the application is closed

This is off until you turn it on. Settings, Notifications, Start at sign-in (Notifier at sign-in in the terminal UI) adds a per-user entry that runs `certwatch notifier run` when you sign in. It needs no administrator rights, opens the database read-only and makes no network connection: it shows alerts that a check has already raised and does not check endpoints itself.

By default a system notification shows only a count. Turn on Show titles in system notifications to add the alert title, which names the host or the certificate.

## Reports

A report is a snapshot of the current data: the certificate inventory, the certificates expiring within 90 days, or the validation and configuration findings. Formats are Markdown, CSV and JSON. Reports are stored in the database with a SHA-256 digest and can be saved as files.

- **Generate report**: `n`, or `certwatch reports generate --kind expiry --format csv`.
- **Save as file**: `o`, or `certwatch reports export <id> --file report.csv`.
- **Remove**: `d`, or `certwatch reports remove <id>`. Files you saved stay.

In CSV reports a cell that starts with `=`, `+`, `-` or `@` is prefixed with an apostrophe so a spreadsheet does not run it as a formula.

## Import and export of the endpoint list

Export list writes every endpoint to a CSV or JSON file that is readable only by your user account. Import list reads a CSV or JSON file with the columns `host`, `port`, `server_name`, `note` and `enabled`; only `host` is required. A preview shows how many endpoints would be added, how many are already monitored and which rows are rejected, with the reason for each. Nothing is written until you confirm, and the import needs the same authorization confirmation as adding an endpoint. Files larger than 4 MB or with more than 5000 rows are refused.

## Settings

Every setting can be changed in the dashboard, in the terminal UI and with `certwatch config set <key> <value>`. Each setting shows where its value comes from: the default, the stored value, an environment variable or a command-line flag. A value that is overridden says by what. Invalid values are refused with the reason and an example. Reset returns one setting to its default.

### Appearance

Choose Light, Dark or Follow system and one of seven color themes: Neutral slate, Blue, Green, Crimson, Amber, Purple and High contrast. A custom accent color is checked against the background and adjusted when it would be hard to read. Severity colors never depend on the theme accent and always come with an icon and a label. Density, text size and reduced motion apply at once.

The terminal UI uses the same themes, works with 16 colors, honors `NO_COLOR` and has a monochrome setting.

### Database location

Settings, Database location shows the file in use. Choose a new location to copy your data there and use it from then on, or switch to a database that already exists. The old file is never deleted. When the location is set by `--db` or by the environment variable, the setting says so and cannot be changed until the override is removed.

## Keyboard

Every action is reachable from the keyboard. About, Shortcuts lists the keys for the dashboard and the terminal UI. In the terminal UI, `:` opens the menu and `?` opens this guide.

## Commands

`certwatch --help` lists every command, and `certwatch <command> --help` explains one. Commands never ask a question when they cannot: scripts and CI get an error that names the flag to add. Exit code 0 means success, 1 means the action failed and 2 means the command line was wrong.

## Privacy

CertWatch has no telemetry, no update check and no account. It makes network connections only for checks: to the hosts you added and confirmed, after showing you the list. On macOS and Windows the operating system validates chains against its trust store and may fetch a missing intermediate certificate or revocation data on its own while it does so.

Links to the project page open in your browser only when you choose them, and a bug report shows its exact text before the browser opens.

## Troubleshooting

- **The dashboard does not open**: on a computer without a display the application opens the terminal UI and says why. `certwatch --ui tui` asks for the terminal UI directly.
- **A check fails with a name lookup or connection error**: the host did not resolve or did not answer within `checks.timeout_seconds`. The endpoint is listed under Failed checks with the reason.
- **The chain of an internal host is not trusted**: add the root certificate of your certificate authority with `checks.trust_file`.
- **The database is busy**: another window is writing. The application waits, retries and tells you. Nothing is lost.
- **A file is not a database**: the application refuses to open or overwrite it. Choose another location.
- **Check the database**: About, Diagnostics or `certwatch db check`.
