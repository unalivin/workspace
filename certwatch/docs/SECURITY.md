# Security properties

How to report a vulnerability is in [../SECURITY.md](../SECURITY.md). The threats and their mitigations are in [THREAT_MODEL.md](THREAT_MODEL.md). This page lists what the application does and does not do.

## Network

- The application has no update check, no telemetry and no remote service, and it listens on no port.
- It opens connections for one purpose: a check of an endpoint the user added. A check is a name lookup, one TCP connection and a TLS handshake. Nothing is sent after the handshake. With protocol probing on, up to three more handshakes test which TLS versions the server accepts.
- An endpoint is stored only after the user confirms being authorized to test the host. The confirmation is needed again when the host or the port changes, and for imports.
- Every action that connects lists its connections first and asks. This question cannot be turned off. Commands need `--yes`; `--dry-run` prints the connections without making them.
- Automatic checks are off by default and run only while the dashboard, the terminal UI or `certwatch watch` is open. The background notifier makes no connection.
- Demo endpoints are never contacted.
- Only the package `internal/tlscheck` may open connections; a test fails when another package does.
- On macOS and Windows the operating system validates chains against its own trust store and may make requests of its own while it does so. See the accepted risks in the threat model.
- The browser opens only after an explicit click and a confirmation that shows the full address, and only for addresses under the configured repository.

## Certificate verification

- Verification is never turned off. A test fails when the source contains the switch that disables it.
- A chain that cannot be verified is recorded with the certificates the server presented and marked as not trusted, with the reason.
- The roots are those of the operating system, plus the PEM file named in `checks.trust_file` when it is set.
- Revocation is not checked.

## Text from servers and files

- Text from certificates and from handshake errors is reduced to printable characters and a bounded length before it is stored or shown. Control characters, escape sequences and direction overrides are replaced.
- Notes lose control characters when they are saved.
- Private keys in imported PEM files are ignored and never stored.

## Files

- The database and the configuration directory are created for the current user only (`0600` and `0700` on Linux and macOS). Exported endpoint lists, certificates and reports are created with mode `0600`, and an existing file is not replaced unless the user asks for it.
- A file that is not a database of this application is never overwritten or changed.
- Moving the database copies it; the old file is left in place.
- The background notifier opens the database read-only.

## Input

- Every SQL statement uses bound parameters. A static check in the test suite fails when a query is built from a non-constant string.
- Imports are limited in size and row count, parsed strictly, and shown as a preview before anything is written.
- In CSV reports a cell that starts with a formula character is prefixed with an apostrophe.
- Text is passed to helper programs (browser, notification tools) as separate arguments, never through a shell.

## Notifications

- A system notification shows a count by default. With Show titles in system notifications it adds the alert title, which names the host or the certificate. Titles are reduced to a safe character set and length. Notes are never included.

## Start at sign-in

- Off by default. The entry is per user, needs no administrator rights and is removed by turning the setting off.
- An executable path with characters that could change the meaning of the entry is refused.

## Diagnostics and bug reports

- Diagnostics stay on the computer. A bug report contains only what its preview shows, and the preview is shown before anything opens. It contains no host names and no paths.

## Dependencies

- Dependencies are pinned in `go.mod` and `go.sum`. `make vuln` runs `govulncheck` and CI runs it on every push.
- `make notices` fails when a dependency has no license file or a license outside the allowed list.

## Not provided

- The database is not encrypted. Use disk encryption if the computer is shared or portable.
- The downloads are not signed. Compare the digest as described in [INSTALLATION.md](INSTALLATION.md).
- Revocation checks, certificate transparency lookups and checks of services that need STARTTLS.
