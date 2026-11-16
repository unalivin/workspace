# 0001. One service layer behind three interfaces

Status: accepted

## Context

The application has a dashboard window, a terminal UI and commands. Users switch between them and expect the same data, the same rules and the same capabilities.

## Decision

All three call the `service` module of `templateapp-core` and the domain modules beside it. The interface crates (`templateapp-app`, `templateapp-tui`, `templateapp-gui`) contain presentation and input handling only: no SQL, no validation rules, no business decisions. The list of capabilities is data (`CAPABILITIES` in `crates/templateapp-core/src/service.rs`); each row names the command and the place in the dashboard and in the terminal UI, tests prove that every row names a real command and that every command has a row, and `docs/COMMANDS.md` is generated from it.

## Consequences

- A feature costs three presentations. The tests refuse a command without a capability row; that the dashboard and the terminal UI really offer each row is checked by their own tests page by page, not yet row by row from the list.
- Messages and validation errors are identical everywhere because they come from one place.
- The no-GUI build is the same program without one crate.
