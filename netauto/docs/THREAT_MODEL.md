# Threat Model

## Assets

- Local SQLite database storing device inventory, reusable jobs, dry-run diffs, approval records, configuration backups, and change logs.
- Encrypted credential store (values encrypted at rest and never displayed in UI or logs).

## Trust boundaries

1. **Device adapters and lab targets:** Mock and lab adapters are used by default; any live target requires explicit configuration and approval before state-changing execution.
2. **Approval gate:** No state-changing job executes without a recorded approval decision.
3. **Credential storage:** Secrets are encrypted before persistence and redacted from execution logs and notifications.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Unapproved configuration changes on devices | Mandatory dry-run preview and explicit approval gate before execution |
| Credential exposure in UI, logs, or exports | Encrypt credentials at rest; never display plaintext values; redact secret patterns in logs |
| SQL injection in inventory or backup queries | Parameterized SQLite queries only |
| Command injection in device adapters | Strict adapter allowlist and argument validation |
