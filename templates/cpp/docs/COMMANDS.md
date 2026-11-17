# Commands and capabilities

Template App has three interfaces over one service layer: the dashboard, the terminal UI and the command line. This page lists what each one can do and every command.

Both tables are generated from the application. After changing a command or a capability run `make docs`; the tests fail when this page is out of date.

## Capability matrix

One row per capability. "n/a" marks a capability that only makes sense on the command line.

<!-- capabilities:start -->
| Capability | Dashboard | Terminal UI | CLI |
|---|---|---|---|
| Create the database on first start | First-start dialog | First-start screen | `db init` |
| Choose the database location | Settings, Data, Database location | Settings, Database location | `db move` |
| Check database health | About, Diagnostics | About, Diagnostics | `db check` |
| See what needs attention | Overview | Overview | `overview` |
| List, search, filter and sort records | Records | Records | `records list` |
| Choose the columns of the record table | Records, Columns | Records, c | `config set` |
| Show one record | Records, row detail panel | Records, Enter | `records show` |
| Add a record | Records, Add record | Records, n | `records add` |
| Edit a record | Records, row detail panel, Edit | Records, e | `records edit` |
| Enable a record | Records, row detail panel, Enable | Records, x | `records enable` |
| Disable a record | Records, row detail panel, Disable | Records, x | `records disable` |
| Remove a record | Records, row detail panel, Remove | Records, d | `records remove` |
| Import records from CSV or JSON | Records, Import | Records, i | `records import` |
| Export records to CSV or JSON | Records, Export | Records, o | `records export` |
| Load labeled demo data | Overview, Load demo data | Overview, l | `demo load` |
| Remove demo data | Settings, Data, Remove demo data | Settings, Remove demo data | `demo remove` |
| List and filter notifications | Notifications | Notifications | `notifications list` |
| Mark notifications as read | Notifications, Mark as read | Notifications, m and M | `notifications read` |
| See the rule of every event type | Notifications, Rules | Notifications, Rules | `notifications rules list` |
| Change a notification rule | Notifications, Rules | Notifications, Rules, e | `notifications rules set` |
| Reset a notification rule | Notifications, Rules, Reset | Notifications, Rules, u | `notifications rules reset` |
| Start the notifier at sign-in | Settings, Notifications, Start at sign-in | Settings, Notifier at sign-in | `notifier install` |
| Stop starting the notifier at sign-in | Settings, Notifications, Start at sign-in | Settings, Notifier at sign-in | `notifier uninstall` |
| See whether the notifier starts at sign-in | Settings, Notifications | Settings, Notifier at sign-in | `notifier status` |
| Run the background notifier | n/a | n/a | `notifier run` |
| See every setting and what overrides it | Settings | Settings | `config list` |
| Change a setting | Settings | Settings, Enter | `config set` |
| Reset one setting | Settings, Reset | Settings, u | `config reset` |
| List color themes | Settings, Appearance, Color theme | Settings, Color theme | `config theme list` |
| Choose a color theme | Settings, Appearance, Color theme | Settings, Color theme | `config theme set` |
| See the theme in use | Settings, Appearance | Settings, Color theme | `config theme show` |
| See the remembered front end | Settings, Launcher | Settings, Open with | `config launcher show` |
| Choose the front end to open by default | Settings, Launcher | Settings, Open with | `config launcher set` |
| Ask again which front end to open | Settings, Launcher | Settings, Open with | `config launcher reset` |
| Reopen in the other front end | Menu, Open terminal UI | Menu, Open dashboard | `--ui` |
| Read and search the usage guide offline | About, Usage guide | About, Usage guide | `guide` |
| Show diagnostics | About, Diagnostics | About, Diagnostics | `diagnostics` |
| Show licenses and notices | About, Licenses and notices | About, Licenses | `licenses` |
| Show the source and bug report links | About, Overview | About, Overview | `about` |
<!-- capabilities:end -->

## Global flags and environment

| Flag | Environment variable | Effect |
|---|---|---|
| `--ui gui` or `--ui tui` | `TEMPLATEAPP_UI` | Open the dashboard or the terminal UI without asking. |
| `--ask` | | Ask which interface to open even when a choice is remembered. |
| `--no-prompt` | | Never ask; open the remembered or default interface. |
| `--db <file>` | `TEMPLATEAPP_DB` | Use this database file for this run. |
| `--set key=value` | `TEMPLATEAPP_<KEY>` | Override one setting for this run. The variable name is the setting key in upper case with dots replaced by underscores, for example `TEMPLATEAPP_APPEARANCE_DENSITY`. The interfaces show that the value is overridden and by what. |
| | `TEMPLATEAPP_CONFIG_DIR` | Directory that holds the saved database path and the saved interface choice. |
| | `NO_COLOR` | Turn colors off in the terminal UI. |

A subcommand never opens the chooser. Without a subcommand, a session that is not interactive (a pipe, a script, CI) prints the help text instead of asking.

Exit codes: `0` success, `1` failure, `2` wrong usage.

## Command reference

<!-- reference:start -->
| Command | Arguments and flags | What it does |
|---|---|---|
| `templateapp overview` | `--format` | Show what needs attention |
| `templateapp records list` | `--all` `--desc` `--format` `--limit` `--search` `--severity` `--sort` `--status` | List, search, filter and sort records |
| `templateapp records show` | `<id>` `--format` | Show one record |
| `templateapp records add` | `--disabled` `--name` `--note` `--severity` `--status` | Add a record |
| `templateapp records edit` | `<id>` `--name` `--note` `--severity` `--status` | Change fields of a record |
| `templateapp records enable` | `<id>` | Enable a record |
| `templateapp records disable` | `<id>` | Disable a record |
| `templateapp records remove` | `<id>` `--dry-run` `--yes` | Remove a record |
| `templateapp records import` | `<file\|->` `--dry-run` `--format` | Import records from a CSV or JSON file, or from standard input with - |
| `templateapp records export` | `--force` `--format` `--output` | Export records as CSV or JSON |
| `templateapp demo load` | none | Add sample records labeled as demo data |
| `templateapp demo remove` | `--dry-run` `--yes` | Remove every demo record |
| `templateapp notifications list` | `--event` `--format` `--limit` `--severity` `--unread` | List notifications, newest first |
| `templateapp notifications read` | `<id\|all>` | Mark one notification, or all of them, as read |
| `templateapp notifications rules list` | none | List the rule of every event type |
| `templateapp notifications rules set` | `<event>` `--dedup-minutes` `--enabled` `--min-severity` `--quiet-end` `--quiet-start` | Change the rule of an event type |
| `templateapp notifications rules reset` | `<event>` | Return the rule of an event type to its defaults |
| `templateapp notifier run` | `--once` | Check for new notifications on an interval until stopped |
| `templateapp notifier install` | `--dry-run` | Start the notifier when you sign in (per-user entry, no administrator rights) |
| `templateapp notifier uninstall` | `--dry-run` | Remove the sign-in entry |
| `templateapp notifier status` | none | Show whether the sign-in entry exists |
| `templateapp config list` | `--format` | List every setting with its value and where the value comes from |
| `templateapp config get` | `<key>` | Print the value of one setting |
| `templateapp config set` | `<key> <value>` | Change a setting |
| `templateapp config reset` | `<key>` | Return one setting to its default |
| `templateapp config theme list` | none | List the color themes |
| `templateapp config theme set` | `<name>` | Choose a color theme |
| `templateapp config theme show` | none | Show the theme, mode and colors in use |
| `templateapp config launcher show` | none | Show the remembered front end |
| `templateapp config launcher set` | `<gui\|tui>` | Remember a front end |
| `templateapp config launcher reset` | none | Forget the choice and ask again |
| `templateapp db init` | none | Create the database if it does not exist |
| `templateapp db path` | none | Print the database path and what decides it |
| `templateapp db move` | `<path>` `--dry-run` `--switch` `--yes` | Copy the database to a new location and use it, or switch to another database |
| `templateapp db check` | none | Run the integrity and foreign-key checks |
| `templateapp diagnostics` | none | Show build, database and settings status |
| `templateapp guide` | `[search terms]` | Read or search the usage guide |
| `templateapp about` | `--issue` | Show version, source and bug report links |
| `templateapp licenses` | none | Show the license and third-party notices |
| `templateapp version` | none | Print the version |
<!-- reference:end -->
