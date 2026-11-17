# 0005. One binary that decides which interface to open

Status: accepted

## Context

Users start the application by double-click, from a terminal, over SSH and from scripts. Separate binaries per interface would triple the downloads and confuse the choice.

## Decision

One binary. The `launcher` module of `secretsentry-core` is a pure function from the arguments, the environment, the remembered choice and a description of the session to one action: open the dashboard, open the terminal UI, ask, show help, or fail with a reason.

- A subcommand never opens an interface.
- `--ui` and the environment variable win over the remembered choice.
- Without a display the dashboard is never chosen; without an interactive terminal nobody is asked.
- From a terminal the question is asked in the terminal; on a desktop start it is a small native window.
- Both interfaces can switch to the other.

On Windows the executable is a console application, so commands and the terminal UI work in the console they are started from and their output is complete before the prompt returns. The price is a console window next to the dashboard when the application is started by double-click.

## Consequences

- Hiding the Windows console window for a double-click start is open work, see [../ROADMAP.md](../ROADMAP.md).
- The decision table is fully testable without any window or terminal.
