# Architecture decision records

Each record states one decision, its context and its consequences. A decision is changed by adding a new record that replaces the old one, not by editing history.

| Record | Decision |
|---|---|
| [0001](0001-one-service-layer.md) | One service layer behind the dashboard, the terminal UI and the commands |
| [0002](0002-settings-in-the-database.md) | Settings live in the database; only the database path and the interface choice live in a file |
| [0003](0003-sqlite-driver-and-pragmas.md) | SQLite through cgo with FTS5, WAL mode and `data_version` polling |
| [0004](0004-fyne-with-own-skin.md) | Fyne for the dashboard, with the toolkit look replaced by design tokens |
| [0005](0005-single-binary-launcher.md) | One binary that decides which interface to open |
| [0006](0006-notifier-without-a-service.md) | A per-user background notifier instead of a system service |
| [0007](0007-no-source-comments.md) | No comments in source files |
