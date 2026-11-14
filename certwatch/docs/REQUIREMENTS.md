# Requirements and acceptance criteria

This page states what CertWatch must do and how each requirement is accepted. The tests behind each requirement are listed in [TESTING.md](TESTING.md); what is missing is in [ROADMAP.md](ROADMAP.md).

## Mission

Monitor the TLS certificates of hosts the user is responsible for. Record expiry, issuer, subject alternative names, chain validation, host name match, protocol observations and history; raise alerts; produce reports. Stay local, cost nothing to run, and never weaken TLS verification.

## Functional requirements

| No. | Requirement | Accepted when |
|---|---|---|
| F1 | Keep a list of endpoints (host, port, optional server name, note, enabled) | An endpoint can be added, edited, enabled, disabled, removed, searched and filtered in all three interfaces |
| F2 | Check an endpoint with one TLS handshake | The check records the chain, issuer, names, validity, key, fingerprint, TLS version and cipher suite of a local test server |
| F3 | Validate the chain and the host name | A trusted chain is valid; an unknown authority, a self-signed certificate, an expired certificate and a wrong name are each reported with the reason |
| F4 | Observe the accepted TLS versions on request | With probing on, a server limited to TLS 1.0 to 1.2 is recorded with exactly these versions and a finding for the old ones |
| F5 | Keep history and detect change | A different leaf certificate at the next check is recorded as a change with both fingerprints; history is kept up to the configured number of observations |
| F6 | Inventory of certificates | Every certificate an enabled endpoint presents is listed with its detail, chain, endpoints and change history; certificates can be imported from and saved to PEM files |
| F7 | Overview of what needs attention | Counts for expired, expiring in 7, 30 and 90 days, chain failures, name mismatches, failed checks and unchecked endpoints; each count opens the list with exactly that many rows; a table of the soonest expiries and the recent alerts |
| F8 | Expiry timeline with zoom | Tracked certificates in the order of expiry with a bar for the time left; zoom to 30, 90, 180 or 365 days or everything |
| F9 | Alerts | One alert per certificate and expiry threshold; one alert when chain or name validation turns from good to bad; one alert when the certificate changes; rules per event type |
| F10 | Reports | Inventory, expiring certificates and findings as Markdown, CSV and JSON with the same rows in each format; stored with a digest; saved as files |
| F11 | Import and export of the endpoint list | CSV and JSON round trip; a preview with accepted, known and rejected rows before anything is written |
| F12 | Automatic checks | Off by default; with an interval set, due endpoints are checked while an interface or `certwatch watch` runs |
| F13 | Private certificate authorities | A PEM file with additional roots makes chains of that authority valid |
| F14 | Demo data | Loaded and removed with one action, labeled in every row, never contacted |

## Safety requirements

| No. | Requirement | Accepted when |
|---|---|---|
| S1 | Only authorized hosts are contacted | Nothing is stored without the authorization confirmation, on add, on change of host or port, and on import |
| S2 | Connections are disclosed | Every network action shows each connection and asks, also with confirmations turned off; commands need `--yes`; `--dry-run` connects to nothing |
| S3 | Verification is never disabled | The switch that disables verification is absent from the source; an untrusted chain is recorded as untrusted |
| S4 | One place opens connections | Only `internal/tlscheck` imports a network client or dials |
| S5 | Server text cannot forge the display | Control characters, escape sequences and direction overrides from certificates and errors are replaced before storage |
| S6 | Bounded work | Every handshake ends within the timeout; imports have size and row limits |
| S7 | Private by default | Files are created for the current user only; no telemetry; system notifications show a count unless titles are turned on; bug reports carry no host names |
| S8 | No destructive surprise | Removals state what is removed and ask; existing files are not replaced without consent; the old database is never deleted by a move |

## Interface requirements

| No. | Requirement | Accepted when |
|---|---|---|
| I1 | Three interfaces over one service layer | Every capability in the matrix of [COMMANDS.md](COMMANDS.md) is reachable in the dashboard, the terminal UI and the commands, proven by a test per interface |
| I2 | Changes appear everywhere | A change made through one interface is visible in another open one without a restart; a conflicting edit is refused and keeps the user's input |
| I3 | Ten categories | Overview, Certificates, Endpoints, Expiry Timeline, Chains, Configuration, Alerts, Reports, About and Settings, in this order, with About and Settings at the bottom |
| I4 | Severity never by color alone | Every severity has an icon or glyph and a label in addition to its color |
| I5 | Keyboard | Every action has a key in the terminal UI and is reachable by keyboard in the dashboard; the shortcuts are listed in About |
| I6 | Usable offline | The usage guide is built in and searchable; nothing is loaded from the network to draw the interface |

## Quality requirements

| No. | Requirement | Accepted when |
|---|---|---|
| Q1 | Runs for free on a normal computer | No paid service, account or key is needed to build, test or run |
| Q2 | Linux, macOS and Windows | CI is green on the three runners for the commit |
| Q3 | No SQL from strings | The static scan passes; hot queries use their indexes |
| Q4 | Honest documents | Tests named in documents exist; the command reference is generated; unverified areas are listed |
| Q5 | Reproducible checks | `make check` runs formatting, static analysis, the tests with the seed inputs of every fuzz target, the race detector, the builds and the document checks; `make fuzz` runs every fuzz target for a set time |
