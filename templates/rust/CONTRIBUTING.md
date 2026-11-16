# Contributing to Template App

Thank you for helping. The short version:

1. Run `scripts/dev-setup.sh` to see which tools are missing; `scripts/dev-setup.sh --install` installs the ones that need no administrator rights.
2. Make the change with a test.
3. Run `make check`.
4. Commit with a Conventional Commits message and open a pull request.

The full guide, with the commit types, the allowed scopes, the branch naming rule and the review checklist, is in [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

Rules that are enforced by tests:

- No comments in Rust, SQL or Bash source files. Express intent through names, small functions and tests, and explain decisions in `docs/`.
- No email address anywhere in the repository. Commit with a GitHub noreply address.
- SQL is written as constants with bound parameters, only in the storage modules.
- Every command has a row in the capability list, every row names a real command, and `docs/COMMANDS.md` is generated from the list.
- A test name written in a document must exist.

Security problems are reported privately, see [SECURITY.md](SECURITY.md).
