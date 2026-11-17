# 0007. No comments in source files

Status: accepted

## Context

Comments drift from the code they describe, and nothing checks them.

## Decision

Rust, SQL and Bash files contain no comments, and that includes documentation comments. Intent is carried by names, small functions, tests that read as specifications, and these records. Attributes such as `#[cfg(...)]` and the interpreter line of scripts are not comments and stay. The test `source_files_have_no_comments` fails when a comment appears.

## Consequences

- Public items have no documentation comments; the crates are not published and are not a public API.
- A reason that does not fit in a name belongs in `docs/ARCHITECTURE.md` or a new record.
- Lint suppressions are attributes in Rust, so they would be possible; none is used. The code is changed until `clippy` is satisfied.
