# 0001. One service layer behind three interfaces

Status: accepted

## Context

The application has a dashboard window, a terminal UI and commands. Users switch between them and expect the same data, the same rules and the same capabilities.

## Decision

All three call the `service` module of `secretsentry-core` and the domain modules beside it. The interface crates (`secretsentry-app`, `secretsentry-tui`, `secretsentry-gui`) contain presentation and input handling only: no SQL, no validation rules, no business decisions. The list of capabilities is data (`CAPABILITIES` in `crates/secretsentry-core/src/service.rs`); each row names the command and the place in the dashboard and in the terminal UI. Tests prove that every row names a real command, that every command has a row, and that the words of every row are painted by the dashboard and shown by the terminal UI. `docs/COMMANDS.md` is generated from the list.

## Consequences

- A feature costs three presentations, and the tests refuse a change that adds it to only one.
- The dashboard and terminal checks look for the labels and keys a row names. They prove that the control is on the screen, not that it does the right thing; that is the job of the tests of each page.
- Messages and validation errors are identical everywhere because they come from one place.
- The no-GUI build is the same program without one crate.
