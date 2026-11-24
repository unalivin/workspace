# Threat Model

## Assets

- Local SQLite database containing ingested log events, saved queries, detection rules, alerts, and audit records.
- Configured ingestion sources (file paths and optional loopback-only ingestion listener).
- Redaction rules that strip secrets and credentials before storage or display.

## Trust boundaries

1. **Untrusted log input:** File imports, syslog lines, JSON lines, and optional loopback API payloads can contain malformed UTF-8, oversized lines, invalid timestamps, or control characters.
2. **Local storage:** SQLite database file on disk, protected by OS file permissions.
3. **Optional ingestion listener:** Disabled by default; when enabled, binds only to `127.0.0.1` with bounded request body size and queue depth.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Memory exhaustion from oversized log lines or unbounded queues | Enforce max line/event size (64 KB) and bounded ingestion queue with backpressure counters |
| SQL injection via search queries or field filters | Parameterized SQL queries only; FTS5 query sanitizer; test fails on string-built SQL |
| Secret leakage in stored events or notifications | Redact matching secret patterns at ingestion and mark redacted fields; notifications never include event payloads |
| Path traversal on file import or export | Canonicalize and validate paths; reject null bytes |
| Unintended network exposure | No outbound network calls; optional listener disabled by default and bound to loopback only |
