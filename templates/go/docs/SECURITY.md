# Security properties

How to report a vulnerability is in [../SECURITY.md](../SECURITY.md). The threats and their mitigations are in [THREAT_MODEL.md](THREAT_MODEL.md). This page lists what the application does and does not do.

## Network

- The application opens no network connection. It has no update check, no telemetry and no remote service.
- The only way out is your browser: a link opens after an explicit click and a confirmation that shows the full address, and only when the address is under the configured repository. Any other address is refused.

## Files

- The database and the configuration directory are created for the current user only (`0600` and `0700` on Linux and macOS). Exported files are created with mode `0600`.
- A file that is not a database of this application is never overwritten or changed.
- Moving the database copies it; the old file is left in place.
- The background notifier opens the database read-only.

## Input

- Every SQL statement uses bound parameters. A static check in the test suite fails when a query is built from a non-constant string.
- Search text is turned into a quoted full-text expression, so it cannot inject query syntax.
- Imports are limited in size and row count, parsed strictly, and shown as a dry run before anything is written.
- Text is passed to helper programs (browser, notification tools) as separate arguments, never through a shell.

## Notifications

- A system notification shows a count by default. With Show titles in system notifications it adds the notification title. Record names and notes are never included, and titles are reduced to a safe character set and length.

## Start at sign-in

- Off by default. The entry is per user, needs no administrator rights and is removed by turning the setting off.
- An executable path with characters that could change the meaning of the entry is refused.

## Diagnostics and bug reports

- Diagnostics stay on the computer. A bug report contains only what its preview shows, and the preview is shown before anything opens.

## Dependencies

- Dependencies are pinned in `go.mod` and `go.sum`. `make vuln` runs `govulncheck` and CI runs it on every push.
- `make notices` fails when a dependency has no license file or a license outside the allowed list.

## Not provided

- The database is not encrypted. Use disk encryption if the computer is shared or portable.
- The downloads are not signed. Compare the digest as described in [INSTALLATION.md](INSTALLATION.md).
