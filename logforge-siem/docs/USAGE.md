# Usage

Run with no arguments to choose between the desktop GUI and terminal UI:

```bash
logforge-app
```

Pass `--ui gui` or `--ui tui` to skip the chooser, or run any subcommand directly:

```bash
logforge-app demo load
logforge-app overview
logforge-app ingest auth-log /var/log/auth.log
logforge-app search "Failed"
logforge-app alerts --status open
```
