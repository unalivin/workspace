# 0008. A check needs an authorized endpoint and a disclosed connection

Status: accepted

## Context

A tool that connects to arbitrary hosts can be pointed at hosts its user may not test, by mistake, through an imported list or on purpose. CertWatch is a monitoring tool for hosts its user is responsible for, not a scanner.

## Decision

- An endpoint is stored only with an explicit confirmation that the user is authorized to test the host. The time of the confirmation is stored with the endpoint. The confirmation is needed again when the host or the port changes, and once for an imported list after its preview.
- An endpoint is one host and one port. There are no address ranges, wildcards or port lists.
- Every action that connects is planned first. The plan lists each connection: the name lookup and the handshake. The interfaces show the plan and ask. This question is independent of the setting that turns off confirmations for destructive actions. Commands need `--yes`, and `--dry-run` prints the plan.
- Automatic checks are off by default. When the user sets an interval they run without a question per run, and only while an interface or `certwatch watch` is open. `certwatch watch` shows the plan once and asks before it starts.
- Probing which TLS versions a server accepts costs up to three more connections and is off by default.
- Demo endpoints use the reserved name `.demo.invalid`, are flagged in the database and are skipped by every check.
- Only `internal/tlscheck` may open connections. `tests/repo_test.go` fails when another package imports a network client or dials.

## Consequences

- A first check takes one more step than in a scanner. That is the intent.
- Scripts must pass `--authorized` and `--yes`. Both are visible in the script.
- The confirmation is the user's statement. The application cannot verify it.
- The background notifier cannot check anything. Alerts for a closed application appear only after the next check.
