# Roadmap

CertWatch is at `0.1.0-dev`. This page lists what does not exist yet. Nothing here is promised for a date.

## Not implemented

Checks:

- Services that start in clear text and upgrade with STARTTLS (SMTP on 25 or 587, IMAP on 143, LDAP on 389). Only ports that start with TLS can be monitored.
- Revocation checks (OCSP, CRL) and certificate transparency lookups. A revoked certificate that is otherwise valid is shown as valid.
- Checks while the application is closed. Automatic checks run while the dashboard, the terminal UI or `certwatch watch` is open. The background notifier only shows alerts that already exist.
- Connections through a proxy, and client certificates.
- An interval, a timeout or expiry thresholds per endpoint. They are settings for all endpoints.
- International domain names in their native spelling. Enter the `xn--` form.
- Which cipher suites a server accepts. Only the negotiated suite is recorded.

Interface:

- A month grid for the Expiry Timeline. The timeline is a list in the order of expiry with a bar for the time left and a zoom.
- Charts. The Overview shows counts and tables.
- Alert delivery outside the computer (mail, chat, webhooks). Alerts appear in the application and, when turned on, as system notifications.
- Copying a row to the clipboard from the terminal UI. The dashboard has Copy; in a terminal use the terminal's own selection or the `show` commands.
- Resizing and reordering table columns with the pointer. The columns and the sort order of the endpoint table are settings; the other tables have fixed columns.
- Saving and restoring the window position. Only the size and the sidebar state are saved.
- Translations. The interface is in English.

Distribution:

- Signed and notarized downloads, an installer, automatic updates.
- Encryption of the database file.
- Builds for Linux on ARM and for macOS on Intel processors.

## Not verified

- Checks against servers on the internet. The tests use local servers with generated certificates.
- Chain validation by the trust store of macOS and Windows. The tests pass their own roots, which is the path Linux always takes.
- The dashboard on a real display, with a screen reader, and on high-DPI screens.
- Display of system notifications, start at sign-in at a real sign-in, and opening a terminal window on each operating system.
- The published release files. They are built and checked by the release workflow, but nobody has installed and started them on a real Linux, macOS or Windows desktop.

See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md) for what the automated checks cover.

## Next

1. Run checks against real servers on Linux, macOS and Windows and record the result in `PLATFORM_SUPPORT.md`.
2. Verify the dashboard by hand on the three systems.
3. STARTTLS for SMTP, IMAP and LDAP.
4. A month grid for the timeline.
