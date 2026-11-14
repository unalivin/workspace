# 0007. No comments in source files

Status: accepted

## Context

Comments drift from the code they describe, and nothing checks them.

## Decision

Go, SQL and Bash files contain no comments. Intent is carried by names, small functions, tests that read as specifications, and these records. Build constraints and the interpreter line of scripts are directives, not comments, and stay. A test fails when a comment appears.

## Consequences

- Exported identifiers have no documentation comments; the packages are internal and are not a public API.
- A reason that does not fit in a name belongs in `docs/ARCHITECTURE.md` or a new record.
- Tool directives that are written as comments, such as linter suppressions, cannot be used; the code is changed until the tool is satisfied.
