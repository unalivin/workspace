# 0009. Certificate verification stays on

Status: accepted

## Context

The application must describe servers whose certificates are expired, self-signed, issued for another name or issued by an unknown authority. The usual shortcut is to turn verification off, read the certificates and verify them by hand. Code that turns verification off tends to be copied and to end up in places where it matters.

## Decision

- The TLS client always verifies the chain and the name. The switch that disables verification does not appear in the source, including the tests; a test fails when it does.
- When verification fails, the handshake error carries the certificates the server presented. The check takes them from that error, records them, and evaluates chain and name separately so the result says which of the two failed and why.
- A result is "valid" only when the TLS library verified it. The application never upgrades a failed verification.
- Tests use a certificate authority generated at run time (`internal/certgen`) and local servers. They pass their own roots to the check; no test certificate is trusted by the operating system and no key is stored in the repository.
- The roots are those of the operating system. A PEM file named in `checks.trust_file` adds roots for private authorities.
- Text taken from certificates and from handshake errors is reduced to printable characters and a bounded length at this boundary, before it reaches the database or an interface.

## Consequences

- A server that fails verification is never sent anything beyond the handshake, which it would not be in any case.
- With the default roots on macOS and Windows the operating system performs the validation and may make requests of its own. This is documented as an accepted risk in `docs/THREAT_MODEL.md`.
- Revocation is not checked.
- Protocol probing reports a version as accepted when the handshake for that version reaches certificate verification, whether or not verification then succeeds.
