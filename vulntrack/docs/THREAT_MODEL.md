# Threat Model

## Assets

- Local SQLite database storing imported scan findings, asset inventory, CVE enrichment metadata, SLA policies, exceptions, and retest records.

## Trust boundaries

1. **Imported scanner reports:** JSON/SARIF scan files may contain malformed fields, oversized strings, or untrusted text.
2. **Enrichment sources:** Offline or configured CVE metadata; no automatic outbound lookups without user action.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Malformed scanner import crashing or exhausting memory | Bounded file/record sizes and strict JSON schema validation |
| Unverified findings presented as confirmed | Explicit status vocabulary separating `reported` from `confirmed` and `retested` |
| SQL injection in finding or asset filters | Parameterized SQLite queries only |
