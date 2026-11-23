# Commands

Run the executable from the `logforge-siem` directory after `cargo build --release -p logforge-app`. Its binary path is `target/release/logforge-app`; the default database is `logforge-siem.db` in the current directory. Use `--db <path>` to select another SQLite file.

| Command | Purpose |
|---|---|
| `logforge-app overview` | Overview counts |
| `logforge-app stats` | Ingestion, source, alert and outbox metrics |
| `logforge-app risk --since <RFC3339> [--limit N]` | Explainable host risk ranking |
| `logforge-app report <severity\|host\|service\|source> [--from RFC3339] [--to RFC3339] [--limit N]` | Event volume report |
| `logforge-app demo load` | Load synthetic end-to-end demo records once into an empty source inventory |
| `logforge-app search <query> [--limit N]` | Search events |
| `logforge-app alerts [--status STATUS]` | List alerts |
| `logforge-app case list [--status STATUS]` | List investigation cases |
| `logforge-app case show <id>` | Show case, notes and linked alerts |
| `logforge-app case create <title> [--description TEXT] [--priority PRIORITY] [--assignee NAME]` | Create a case |
| `logforge-app case status <id> <status>` | Change case status |
| `logforge-app case assign <id> <assignee>` | Assign a case |
| `logforge-app case note <id> <author> <body>` | Add a case note |
| `logforge-app case link-alert <id> <alert-id>` | Link an alert to a case |
| `logforge-app case unlink-alert <id> <alert-id>` | Remove an alert link |
| `logforge-app case-metrics [--overdue-hours N]` | Case queue and assignment metrics |
| `logforge-app case-export <id> [--redacted]` | Export a case evidence bundle |
| `logforge-app sources` | List configured sources |
| `logforge-app rules` | List detection rules |
| `logforge-app rule-preview [--limit N] [--query QUERY]` | Preview rule matches without writing alerts |
| `logforge-app validate <file> [--format FORMAT] [--max-line-bytes N] [--error-samples N]` | Validate a log file without storing it |
| `logforge-app ingest <source> <file> [--format FORMAT]` | Ingest a configured local file |
| `logforge-app config theme list\|show\|set <name>` | Read or update the saved theme preference |
| `logforge-app notifier <status\|install\|uninstall\|run>` | Inspect or control the notifier command surface |
| `logforge-app outbox [--state STATE] [--limit N] [--before-id ID]` | Page through notification metadata without payloads |
| `logforge-app audit-verify` | Verify the audit hash chain |
| `logforge-app audit-log [--actor NAME] [--action ACTION] [--limit N]` | Query audit history |
| `logforge-app diagnostics` | Read-only diagnostics snapshot |
| `logforge-app retention-set <name> --days N --events N --bytes N [--disabled]` | Update bounded retention policy |
| `logforge-app retention [--prune N] [--max-bytes N] [--dry-run]` | Explain retention candidates or prune them |
| `logforge-app export [--format json\|csv]` | Export up to 500 recent events |
| `logforge-app --ui gui` | Launch desktop dashboard |
| `logforge-app --ui tui` | Launch terminal dashboard |

The GUI and TUI are local views of the same SQLite data. They do not run network listeners. Network delivery is not configured by default; `notifier run` reports queue state rather than claiming delivery when no adapter exists.
