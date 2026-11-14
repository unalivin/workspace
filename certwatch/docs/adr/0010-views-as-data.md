# 0010. Views and their actions are data

Status: accepted

## Context

Six categories are tables with a filter, a search, a detail and a set of actions: Certificates, Endpoints, Expiry Timeline, Chains, Configuration and Reports. Writing each of them three times, once per interface, invites differences in wording, in confirmation rules and in which actions exist.

## Decision

`internal/app/views.go` describes each view as data: its filters, its columns and its actions. An action has an identifier, a label, a key, the fields of its form and three flags: it changes or removes data that cannot be restored, it connects to the network, or it has a preview.

The service layer offers the same five calls for every view: the table, the detail of a row, the default values of a form, the plan of an action and its execution. A plan states in words what will happen and lists the network connections. The interfaces render tables, forms and plans generically:

- the commands are generated from the views, with one sub-command per action and one flag per field
- the terminal UI binds the key of each action and draws the form
- the dashboard creates a button per action and a dialog per form

## Consequences

- An action added to a view appears in all three interfaces, in the capability matrix and in `docs/COMMANDS.md`. Tests in each interface fail when an action has no command, key or button.
- Wording, validation and confirmation rules cannot differ between interfaces.
- A view that needs a layout of its own, such as the Overview and the Alerts inbox, is written by hand in each interface and is covered by the capability tests instead.
- The generic table is less tailored than a hand-written page. Column widths are computed, not designed per view.
