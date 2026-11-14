# Commands and capabilities

CertWatch has three interfaces over one service layer: the dashboard, the terminal UI and the command line. This page lists what each one can do and every command.

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
| List, search and filter endpoints | Endpoints | Endpoints | `endpoints list` |
| Choose the columns and the sort order of the endpoint table | Settings, Tables | Settings, Tables | `config set` |
| Show one endpoint with findings, chain and history | Endpoints, row detail panel | Endpoints, Enter | `endpoints show` |
| Add an endpoint after confirming authorization | Endpoints, Add endpoint | Endpoints, n | `endpoints add` |
| Edit an endpoint | Endpoints, Edit | Endpoints, e | `endpoints edit` |
| Enable an endpoint | Endpoints, Enable | Endpoints, x | `endpoints enable` |
| Disable an endpoint | Endpoints, Disable | Endpoints, X | `endpoints disable` |
| Remove an endpoint with its history | Endpoints, Remove | Endpoints, d | `endpoints remove` |
| Check one endpoint over the network after confirmation | Endpoints, Check now | Endpoints, c | `endpoints check` |
| Check every enabled endpoint after confirmation | Endpoints, Check all | Endpoints, C | `endpoints check-all` |
| Import endpoints from CSV or JSON | Endpoints, Import list | Endpoints, i | `endpoints import` |
| Export endpoints to CSV or JSON | Endpoints, Export list | Endpoints, o | `endpoints export` |
| Check endpoints automatically at an interval | Settings, Checks, Automatic check interval | Settings, Automatic check interval | `watch` |
| List, search and filter certificates | Certificates | Certificates | `certificates list` |
| Show one certificate with names, chain and change history | Certificates, row detail panel | Certificates, Enter | `certificates show` |
| Track the certificates of a PEM file | Certificates, Import PEM file | Certificates, i | `certificates import` |
| Save a certificate as a PEM file | Certificates, Save as PEM | Certificates, o | `certificates export` |
| Stop tracking an imported certificate | Certificates, Stop tracking | Certificates, d | `certificates stop-tracking` |
| See the expiry timeline and change its zoom | Expiry Timeline, Zoom | Expiry Timeline, f | `timeline list` |
| Show one certificate of the timeline | Expiry Timeline, row detail panel | Expiry Timeline, Enter | `timeline show` |
| See the chain validation result of every endpoint | Chains | Chains | `chains list` |
| See who issued each certificate of a chain | Chains, row detail panel | Chains, Enter | `chains show` |
| See negotiated and accepted TLS versions | Configuration | Configuration | `configuration list` |
| See the findings of one endpoint | Configuration, row detail panel | Configuration, Enter | `configuration show` |
| Probe which TLS versions an endpoint accepts | Configuration, Probe TLS versions | Configuration, c | `configuration probe` |
| List reports | Reports | Reports | `reports list` |
| Read a report | Reports, row detail panel | Reports, Enter | `reports show` |
| Generate an inventory, expiry or findings report | Reports, Generate report | Reports, n | `reports generate` |
| Save a report as a file | Reports, Save as file | Reports, o | `reports export` |
| Remove a report | Reports, Remove | Reports, d | `reports remove` |
| Set the expiry thresholds | Settings, Alerts, Expiry thresholds | Settings, Expiry thresholds | `config set` |
| Load labeled demo data | Overview, Load demo data | Overview, l | `demo load` |
| Remove demo data | Settings, Data, Remove demo data | Settings, Remove demo data | `demo remove` |
| List and filter alerts | Alerts | Alerts | `alerts list` |
| Mark alerts as read | Alerts, Mark as read | Alerts, m and M | `alerts read` |
| See the rule of every event type | Alerts, Rules | Alerts, Rules | `alerts rules list` |
| Change an alert rule | Alerts, Rules | Alerts, Rules, e | `alerts rules set` |
| Reset an alert rule | Alerts, Rules, Reset | Alerts, Rules, u | `alerts rules reset` |
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
| `--ui gui` or `--ui tui` | `CERTWATCH_UI` | Open the dashboard or the terminal UI without asking. |
| `--ask` | | Ask which interface to open even when a choice is remembered. |
| `--no-prompt` | | Never ask; open the remembered or default interface. |
| `--db <file>` | `CERTWATCH_DB` | Use this database file for this run. |
| `--set key=value` | `CERTWATCH_<KEY>` | Override one setting for this run. The variable name is the setting key in upper case with dots replaced by underscores, for example `CERTWATCH_APPEARANCE_DENSITY`. The interfaces show that the value is overridden and by what. |
| | `CERTWATCH_CONFIG_DIR` | Directory that holds the saved database path and the saved interface choice. |
| | `NO_COLOR` | Turn colors off in the terminal UI. |

A subcommand never opens the chooser. Without a subcommand, a session that is not interactive (a pipe, a script, CI) prints the help text instead of asking.

Exit codes: `0` success, `1` failure, `2` wrong usage.

## Command reference

<!-- reference:start -->
| Command | Arguments and flags | What it does |
|---|---|---|
| `certwatch overview` | `--format` | Show what expires soon and what failed validation |
| `certwatch certificates list` | `--filter` `--format` `--search` | List the certificates table |
| `certwatch certificates show` | `<id>` `--format` | Show one entry in full |
| `certwatch certificates import` | `--dry-run` `--file` | Track the certificates of a PEM file. Private keys in the file are ignored and never stored |
| `certwatch certificates export` | `<id>` `--file` `--overwrite` | Write the selected certificate to a PEM file |
| `certwatch certificates stop-tracking` | `<id>` `--dry-run` `--yes` | Remove an imported certificate from the inventory |
| `certwatch endpoints list` | `--filter` `--format` `--search` | List the endpoints table |
| `certwatch endpoints show` | `<id>` `--format` | Show one entry in full |
| `certwatch endpoints add` | `--authorized` `--host` `--note` `--port` `--server-name` | Monitor a host you are authorized to test |
| `certwatch endpoints check` | `<id>` `--dry-run` `--yes` | Connect to the selected endpoint and record what it presents |
| `certwatch endpoints check-all` | `--dry-run` `--yes` | Connect to every enabled endpoint and record what it presents |
| `certwatch endpoints edit` | `<id>` `--authorized` `--enabled` `--host` `--note` `--port` `--server-name` | Change the selected endpoint |
| `certwatch endpoints enable` | `<id>` | Include the selected endpoint in checks, counts and alerts |
| `certwatch endpoints disable` | `<id>` | Leave the selected endpoint out of checks, counts and alerts |
| `certwatch endpoints remove` | `<id>` `--dry-run` `--yes` | Remove the selected endpoint with its observation history |
| `certwatch endpoints import` | `--authorized` `--dry-run` `--file` | Add endpoints from a CSV or JSON file with the columns host, port, server_name, note and enabled |
| `certwatch endpoints export` | `--file` `--overwrite` | Write every endpoint to a CSV or JSON file |
| `certwatch timeline list` | `--format` `--search` `--zoom` | List the expiry timeline table |
| `certwatch timeline show` | `<id>` `--format` | Show one entry in full |
| `certwatch chains list` | `--filter` `--format` `--search` | List the chains table |
| `certwatch chains show` | `<id>` `--format` | Show one entry in full |
| `certwatch configuration list` | `--filter` `--format` `--search` | List the configuration table |
| `certwatch configuration show` | `<id>` `--format` | Show one entry in full |
| `certwatch configuration probe` | `<id>` `--dry-run` `--yes` | Connect to the selected endpoint once per TLS version to see which versions it accepts |
| `certwatch reports list` | `--filter` `--format` `--search` | List the reports table |
| `certwatch reports show` | `<id>` `--format` | Show one entry in full |
| `certwatch reports generate` | `--format` `--kind` | Create a report from the current data |
| `certwatch reports export` | `<id>` `--file` `--overwrite` | Write the selected report to a file |
| `certwatch reports remove` | `<id>` `--dry-run` `--yes` | Remove the selected report from the database. Saved files stay |
| `certwatch watch` | `--dry-run` `--interval` `--once` `--yes` | Check enabled endpoints at the configured interval until interrupted |
| `certwatch demo load` | none | Add sample endpoints and certificates labeled as demo data |
| `certwatch demo remove` | `--dry-run` `--yes` | Remove every demo endpoint, certificate and alert |
| `certwatch alerts list` | `--event` `--format` `--limit` `--severity` `--unread` | List notifications, newest first |
| `certwatch alerts read` | `<id\|all>` | Mark one notification, or all of them, as read |
| `certwatch alerts rules list` | none | List the rule of every event type |
| `certwatch alerts rules set` | `<event>` `--dedup-minutes` `--enabled` `--min-severity` `--quiet-end` `--quiet-start` | Change the rule of an event type |
| `certwatch alerts rules reset` | `<event>` | Return the rule of an event type to its defaults |
| `certwatch notifier run` | `--once` | Check for new notifications on an interval until stopped |
| `certwatch notifier install` | `--dry-run` | Start the notifier when you sign in (per-user entry, no administrator rights) |
| `certwatch notifier uninstall` | `--dry-run` | Remove the sign-in entry |
| `certwatch notifier status` | none | Show whether the sign-in entry exists |
| `certwatch config list` | `--format` | List every setting with its value and where the value comes from |
| `certwatch config get` | `<key>` | Print the value of one setting |
| `certwatch config set` | `<key> <value>` | Change a setting |
| `certwatch config reset` | `<key>` | Return one setting to its default |
| `certwatch config theme list` | none | List the color themes |
| `certwatch config theme set` | `<name>` | Choose a color theme |
| `certwatch config theme show` | none | Show the theme, mode and colors in use |
| `certwatch config launcher show` | none | Show the remembered front end |
| `certwatch config launcher set` | `<gui\|tui>` | Remember a front end |
| `certwatch config launcher reset` | none | Forget the choice and ask again |
| `certwatch db init` | none | Create the database if it does not exist |
| `certwatch db path` | none | Print the database path and what decides it |
| `certwatch db move` | `<path>` `--dry-run` `--switch` `--yes` | Copy the database to a new location and use it, or switch to another database |
| `certwatch db check` | none | Run the integrity and foreign-key checks |
| `certwatch diagnostics` | none | Show build, database and settings status |
| `certwatch guide` | `[search terms]` | Read or search the usage guide |
| `certwatch about` | `--issue` | Show version, source and bug report links |
| `certwatch licenses` | none | Show the license and third-party notices |
| `certwatch version` | none | Print the version |
<!-- reference:end -->
