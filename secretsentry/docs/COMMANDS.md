# Commands and capabilities

SecretSentry has three interfaces over one service layer: the dashboard, the terminal UI and the command line. This page lists what each one can do and every command.

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
| Scan a directory, its Git history or both | n/a | Scans, n | `scan` |
| See earlier scan runs | n/a | Scans | `scans list` |
| Show one scan run | n/a | Scans, Enter | `scans show` |
| List, search, filter and sort findings | n/a | Findings | `findings list` |
| Show one finding with its reasons and places | n/a | Findings, Enter | `findings show` |
| Record a decision and a note for a finding | n/a | Findings, e | `findings decide` |
| Export findings as JSON or SARIF | n/a | Exports, o | `export` |
| See findings by the commit that added them | n/a | Git History | `history list` |
| Count new and baseline findings | n/a | Overview, In the baseline | `baseline status` |
| Accept the current findings as the baseline | n/a | Findings, B | `baseline accept` |
| Empty the baseline | n/a | Findings, R | `baseline reset` |
| List detector rules | n/a | Rules | `rules list` |
| Show one detector rule | n/a | Rules, Enter | `rules show` |
| Add a custom rule | n/a | Rules, n | `rules add` |
| Edit a custom rule | n/a | Rules, e | `rules edit` |
| Enable a rule | n/a | Rules, x | `rules enable` |
| Disable a rule | n/a | Rules, x | `rules disable` |
| Return a built-in rule to its built-in state | n/a | Rules, u | `rules restore` |
| Remove a custom rule | n/a | Rules, d | `rules remove` |
| Try a rule on sample text | n/a | Rules, t | `rules try` |
| List allowlist entries | n/a | Allowlists | `allowlist list` |
| Add an allowlist entry | n/a | Allowlists, n | `allowlist add` |
| Edit an allowlist entry | n/a | Allowlists, e | `allowlist edit` |
| Enable an allowlist entry | n/a | Allowlists, x | `allowlist enable` |
| Disable an allowlist entry | n/a | Allowlists, x | `allowlist disable` |
| Remove an allowlist entry | n/a | Allowlists, d | `allowlist remove` |
| List, search, filter and sort records | Records | n/a | `records list` |
| Choose the columns of the record table | Records, Columns | n/a | `config set` |
| Show one record | Records, row detail panel | n/a | `records show` |
| Add a record | Records, Add record | n/a | `records add` |
| Edit a record | Records, row detail panel, Edit | n/a | `records edit` |
| Enable a record | Records, row detail panel, Enable | n/a | `records enable` |
| Disable a record | Records, row detail panel, Disable | n/a | `records disable` |
| Remove a record | Records, row detail panel, Remove | n/a | `records remove` |
| Import records from CSV or JSON | Records, Import | n/a | `records import` |
| Export records to CSV or JSON | Records, Export | n/a | `records export` |
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
| `--ui gui` or `--ui tui` | `SECRETSENTRY_UI` | Open the dashboard or the terminal UI without asking. |
| `--ask` | | Ask which interface to open even when a choice is remembered. |
| `--no-prompt` | | Never ask; open the remembered or default interface. |
| `--db <file>` | `SECRETSENTRY_DB` | Use this database file for this run. |
| `--set key=value` | `SECRETSENTRY_<KEY>` | Override one setting for this run. The variable name is the setting key in upper case with dots replaced by underscores, for example `SECRETSENTRY_APPEARANCE_DENSITY`. The interfaces show that the value is overridden and by what. |
| | `SECRETSENTRY_CONFIG_DIR` | Directory that holds the saved database path and the saved interface choice. |
| | `NO_COLOR` | Turn colors off in the terminal UI. |

A subcommand never opens the chooser. Without a subcommand, a session that is not interactive (a pipe, a script, CI) prints the help text instead of asking.

Exit codes: `0` success, `1` failure, `2` wrong usage.

## Command reference

<!-- reference:start -->
| Command | Arguments and flags | What it does |
|---|---|---|
| `secretsentry overview` | `--format` | Show what needs attention |
| `secretsentry scan` | `<directory>` `--baseline` `--fail-on` `--force` `--format` `--max-commits` `--max-file-kib` `--output` `--source` `--write-baseline` | Scan a directory, its Git history or both for likely secrets |
| `secretsentry scans list` | `--format` `--kind` `--limit` | List scan runs, newest first |
| `secretsentry scans show` | `<id>` `--format` | Show one scan run |
| `secretsentry findings list` | `--commit` `--confidence` `--format` `--limit` `--min-score` `--offset` `--order` `--path` `--root` `--rule` `--scan` `--scope` `--search` `--sort` `--status` | List, search, filter and sort findings |
| `secretsentry findings show` | `<id>` `--format` | Show one finding with its reasons and places |
| `secretsentry findings decide` | `<id>` `--note` `--status` | Record a decision and a note for a finding |
| `secretsentry history list` | `--format` `--limit` | List commits that added findings, newest first |
| `secretsentry baseline status` | `--root` | Count new and baseline findings |
| `secretsentry baseline accept` | `--root` | Mark every current finding as known |
| `secretsentry baseline reset` | `--dry-run` `--root` `--yes` | Empty the baseline so that every finding counts as new |
| `secretsentry rules list` | `--format` | List built-in and custom rules |
| `secretsentry rules show` | `<rule>` `--format` | Show one rule with its pattern |
| `secretsentry rules add` | `<rule>` `--confidence` `--description` `--disabled` `--entropy` `--group` `--keywords` `--name` `--pattern` `--severity` | Add a custom rule |
| `secretsentry rules edit` | `<rule>` `--confidence` `--description` `--entropy` `--group` `--keywords` `--name` `--pattern` `--severity` | Change fields of a custom rule |
| `secretsentry rules enable` | `<rule>` | Enable a rule |
| `secretsentry rules disable` | `<rule>` | Disable a rule |
| `secretsentry rules restore` | `<rule>` | Return a built-in rule to its built-in state |
| `secretsentry rules remove` | `<rule>` `--dry-run` `--yes` | Remove a custom rule |
| `secretsentry rules try` | `[rule]` `--confidence` `--description` `--entropy` `--file` `--group` `--keywords` `--name` `--pattern` `--severity` `--text` | Try a rule on sample text without saving anything |
| `secretsentry allowlist list` | `--format` | List allowlist entries |
| `secretsentry allowlist add` | `--disabled` `--kind` `--note` `--pattern` `--rule` | Add an allowlist entry |
| `secretsentry allowlist edit` | `<id>` `--kind` `--note` `--pattern` `--rule` | Change fields of an allowlist entry |
| `secretsentry allowlist enable` | `<id>` | Enable an allowlist entry |
| `secretsentry allowlist disable` | `<id>` | Disable an allowlist entry |
| `secretsentry allowlist remove` | `<id>` `--dry-run` `--yes` | Remove an allowlist entry |
| `secretsentry export` | `<file\|->` `--commit` `--confidence` `--force` `--format` `--min-score` `--path` `--root` `--rule` `--scan` `--scope` `--search` `--status` | Write findings as JSON or SARIF with redacted values |
| `secretsentry records list` | `--all` `--desc` `--format` `--limit` `--search` `--severity` `--sort` `--status` | List, search, filter and sort records |
| `secretsentry records show` | `<id>` `--format` | Show one record |
| `secretsentry records add` | `--disabled` `--name` `--note` `--severity` `--status` | Add a record |
| `secretsentry records edit` | `<id>` `--name` `--note` `--severity` `--status` | Change fields of a record |
| `secretsentry records enable` | `<id>` | Enable a record |
| `secretsentry records disable` | `<id>` | Disable a record |
| `secretsentry records remove` | `<id>` `--dry-run` `--yes` | Remove a record |
| `secretsentry records import` | `<file\|->` `--dry-run` `--format` | Import records from a CSV or JSON file, or from standard input with - |
| `secretsentry records export` | `--force` `--format` `--output` | Export records as CSV or JSON |
| `secretsentry demo load` | none | Add an invented scan with findings labeled as demo data |
| `secretsentry demo remove` | `--dry-run` `--yes` | Remove every demo scan, finding and allowlist entry |
| `secretsentry notifications list` | `--event` `--format` `--limit` `--severity` `--unread` | List notifications, newest first |
| `secretsentry notifications read` | `<id\|all>` | Mark one notification, or all of them, as read |
| `secretsentry notifications rules list` | none | List the rule of every event type |
| `secretsentry notifications rules set` | `<event>` `--dedup-minutes` `--enabled` `--min-severity` `--quiet-end` `--quiet-start` | Change the rule of an event type |
| `secretsentry notifications rules reset` | `<event>` | Return the rule of an event type to its defaults |
| `secretsentry notifier run` | `--once` | Check for new notifications on an interval until stopped |
| `secretsentry notifier install` | `--dry-run` | Start the notifier when you sign in (per-user entry, no administrator rights) |
| `secretsentry notifier uninstall` | `--dry-run` | Remove the sign-in entry |
| `secretsentry notifier status` | none | Show whether the sign-in entry exists |
| `secretsentry config list` | `--format` | List every setting with its value and where the value comes from |
| `secretsentry config get` | `<key>` | Print the value of one setting |
| `secretsentry config set` | `<key> <value>` | Change a setting |
| `secretsentry config reset` | `<key>` | Return one setting to its default |
| `secretsentry config theme list` | none | List the color themes |
| `secretsentry config theme set` | `<name>` | Choose a color theme |
| `secretsentry config theme show` | none | Show the theme, mode and colors in use |
| `secretsentry config launcher show` | none | Show the remembered front end |
| `secretsentry config launcher set` | `<gui\|tui>` | Remember a front end |
| `secretsentry config launcher reset` | none | Forget the choice and ask again |
| `secretsentry db init` | none | Create the database if it does not exist |
| `secretsentry db path` | none | Print the database path and what decides it |
| `secretsentry db move` | `<path>` `--dry-run` `--switch` `--yes` | Copy the database to a new location and use it, or switch to another database |
| `secretsentry db check` | none | Run the integrity and foreign-key checks |
| `secretsentry diagnostics` | none | Show build, database and settings status |
| `secretsentry guide` | `[search terms]` | Read or search the usage guide |
| `secretsentry about` | `--issue` | Show version, source and bug report links |
| `secretsentry licenses` | none | Show the license and third-party notices |
| `secretsentry version` | none | Print the version |
<!-- reference:end -->
