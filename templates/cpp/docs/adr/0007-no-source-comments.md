# 0007. No comments in source files

Status: accepted

## Context

Comments drift from the code they describe, and nothing checks them.

## Decision

C++, CMake, SQL and Bash files contain no comments, and that includes documentation comments. Intent is carried by names, small functions, tests whose names are sentences, and these records. Preprocessor lines such as `#include` and the interpreter line of scripts are not comments and stay. The vendored SQLite in `third_party/` is left as published. The test `"source files have no comments"` fails when a comment appears.

## Consequences

- Headers have no documentation comments; the libraries are not installed and are not a public API.
- A reason that does not fit in a name belongs in `docs/ARCHITECTURE.md` or a new record.
- Suppressions of `clang-tidy` and `cppcheck` are written as comments, so none can exist. The code is changed until the tools are satisfied, or a check is switched off for the whole project in `.clang-tidy` with the reason in [../TESTING.md](../TESTING.md).
