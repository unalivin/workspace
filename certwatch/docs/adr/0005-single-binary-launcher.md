# 0005. One binary that decides which interface to open

Status: accepted

## Context

Users start the application by double-click, from a terminal, over SSH and from scripts. Separate binaries per interface would triple the downloads and confuse the choice.

## Decision

One binary. `internal/launcher` is a pure function from the arguments, the environment, the remembered choice and a description of the session to one action: open the dashboard, open the terminal UI, ask, show help, or fail with a reason.

- A subcommand never opens an interface.
- `--ui` and the environment variable win over the remembered choice.
- Without a display the dashboard is never chosen; without an interactive terminal nobody is asked.
- From a terminal the question is asked in the terminal; on a desktop start it is a small native window.
- Both interfaces can switch to the other.

On Windows the release executable is a window application so a double-click shows no console. It attaches to the parent console when there is one and creates one when the terminal UI is requested without a console.

## Consequences

- Command output on Windows is written to an attached console, so the prompt returns before the output is complete.
- The decision table is fully testable without any window or terminal.
