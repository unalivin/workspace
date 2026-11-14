# 0001. One service layer behind three interfaces

Status: accepted

## Context

The application has a dashboard window, a terminal UI and commands. Users switch between them and expect the same data, the same rules and the same capabilities.

## Decision

All three call `internal/app` and the domain packages below it. Interface packages contain presentation and input handling only: no SQL, no validation rules, no business decisions. The list of capabilities is data (`internal/app/capabilities.go`); a test in each interface package proves that every row is reachable there, and `docs/COMMANDS.md` is generated from it.

## Consequences

- A feature costs three presentations, and the tests refuse a change that adds it to only one.
- Messages and validation errors are identical everywhere because they come from one place.
- The no-GUI build is the same program without one package.
