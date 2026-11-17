# Requirements and acceptance criteria

This page states what SecretSentry must do and how each requirement is accepted. The tests behind each requirement are listed in [TESTING.md](TESTING.md); what is missing is in [ROADMAP.md](ROADMAP.md). The threats behind the security requirements are in [THREAT_MODEL.md](THREAT_MODEL.md).

## Mission

Find likely credentials in source trees and in Git history with tuned detector rules, show why each finding was reported, and keep the triage decision. Stay local, cost nothing to run, never write a full secret to any output or to the database, and never send repository content anywhere.

## Words used on this page

| Word | Meaning |
|---|---|
| Rule | A detector: a pattern, the part of the match that is the secret, optional keywords, an optional entropy threshold, a severity and a starting confidence |
| Finding | One secret value reported by one rule in one file path. The same value in the same path found again by a later scan is the same finding |
| Occurrence | A place where a finding was seen: a line and column of a file, and for history scans the commit that introduced it |
| Fingerprint | A SHA-256 digest of the database key, the rule, the path and the secret value. It identifies a finding without storing the value |
| Preview | The redacted form of a value: at most its first four characters followed by a fixed mask, and its length |
| Allowlist entry | A reason to treat matching findings as known test or example data: a path pattern, a value pattern or a fingerprint |
| Baseline | The findings that were known when the user last accepted the current state. Findings that appear later are new |

## Functional requirements

| No. | Requirement | Accepted when |
|---|---|---|
| F1 | Detect secrets with a built-in rule set for common credential formats | Each built-in rule reports its positive fixtures and reports nothing for its negative fixtures; the rule table in the usage guide lists exactly the built-in rules |
| F2 | Entropy check only where a format alone is not specific enough | A rule with a threshold reports a random value and drops a dictionary word of the same length; rules for fixed formats have no threshold |
| F3 | Contextual confidence with reasons | Every finding has a score from 0 to 100, a level (low, medium, high) and the list of reasons with their effect; a placeholder value, a test path and a missing keyword each lower the score by the documented amount |
| F4 | Redaction in every output | No output of any interface, no log line, no export file, no notification and no database column contains a detected value; a test scans the database file and every output of a fixture scan for the fixture values |
| F5 | Scan a directory tree | A scan of a fixture tree reports the expected findings with path, line and column; binary files, oversized files, symbolic links and skipped directories are counted and not read; the result is the same on every run |
| F6 | Scan Git history without running Git | A scan of a fixture repository with loose objects, a pack file and deltas reports a secret that was committed and later removed, with the commit that introduced it; no program is started |
| F7 | Stable findings across scans | A second scan of unchanged content adds no finding; a moved line keeps the finding; a finding that is no longer seen in a tree scan is marked as gone and keeps its history |
| F8 | Triage decisions | A finding can be marked open, confirmed, false positive, accepted risk or resolved with a note in all three interfaces; the decision survives later scans |
| F9 | Baseline | Accepting the baseline marks every current finding as known; a later scan counts only other findings as new; a baseline file can be written and used by another database |
| F10 | Manage detector rules | A custom rule can be added, edited, enabled, disabled and removed in all three interfaces; a built-in rule can be disabled and restored but not removed; an invalid pattern is refused with the position of the error; a rule can be tried on sample text before saving |
| F11 | Manage allowlists | A path pattern, a value pattern and a fingerprint entry can be added, edited, enabled, disabled and removed in all three interfaces; a matching finding is kept, marked as allowlisted with the entry that matched, and not counted as new; the line marker `secretsentry:allow` does the same for one line |
| F12 | Overview of what needs attention | Findings by rule and confidence, new versus baseline, the files with the most findings, and the duration and scope of the last scan; each number opens the list with exactly that many rows |
| F13 | Findings table and detail | The table is sortable and filterable by rule, confidence, status, new or baseline and text; the detail shows the redacted context line, the reasons, the occurrences and the commit when there is one |
| F14 | Git History view | History findings grouped by the commit that introduced them, with author date and subject |
| F15 | Exports | Findings as JSON and as SARIF 2.1.0 with the same findings in both; values are redacted; a file is written with owner-only permissions and never overwritten without confirmation |
| F16 | Use in continuous integration | `secretsentry scan` returns exit code 3 when findings at or above the chosen confidence exist and are not in the baseline, 0 when there are none, and prints nothing but the report when the output is JSON or SARIF |
| F17 | Notifications | One notification when a scan finds new secrets, with the count and never a value; one when a scan fails; rules per event type |
| F18 | Demo data | A labeled demo scan with invented findings can be loaded and removed without touching real data |

## Shell requirements

| No. | Requirement | Accepted when |
|---|---|---|
| S1 | Launcher | No arguments: chooser on a desktop, numbered prompt in a terminal, terminal interface on a headless session; `--ui`, `--ask`, `--no-prompt` and `SECRETSENTRY_UI`; a subcommand never prompts |
| S2 | Navigation | Sidebar with Overview, Scans, Findings, Git History, Rules, Allowlists, Exports, Notifications, About and Settings, each with an icon; keyboard shortcuts for categories, search, Settings and the usage guide |
| S3 | Everything is configurable in the application | Every capability has a row in the capability matrix and is present in the dashboard, the terminal interface and the command line where it applies; a test fails when one is missing |
| S4 | Dashboard and terminal stay in sync | A change made through one interface is visible in the others; the dashboard notices outside changes within about two seconds |
| S5 | Themes | Light, dark and follow system; seven named themes and a custom accent with a contrast check; the same themes in the terminal interface; `NO_COLOR` and monochrome |
| S6 | About | Overview, usage guide, shortcuts, diagnostics, licenses; Source Code and bug report links open only after confirmation |
| S7 | Notifier | `secretsentry notifier run`, `install`, `uninstall` and `status`; off by default; reads the database read-only |
| S8 | Application icon | A key silhouette crossed by a redaction bar, as editable SVG with every required size and format |

## Security requirements

| No. | Requirement | Accepted when |
|---|---|---|
| Q1 | A full secret value is never stored | The database holds the fingerprint, the preview and the length only; the context line is stored with every detected value masked and with long random-looking words masked |
| Q2 | No network access | The scanner opens no socket; the only network action of the application is opening a repository link in the browser after confirmation |
| Q3 | Scans stay inside the chosen scope | A scan reads only below the directory the user named; symbolic links are not followed; a history scan reads only the object store of that repository |
| Q4 | Untrusted repositories cannot run code | History is read from the object files directly; no hook, filter, pager or configured command of the scanned repository is run |
| Q5 | Bounded work | File size, object size, delta chain length, directory depth, number of commits and findings per file are limited; a scan can be cancelled; a pattern cannot take more than linear time |
| Q6 | Malformed input is refused, not trusted | Damaged pack files, indexes, objects, rule files, allowlist patterns and baseline files produce an error message and no crash; every parser has a fuzz target or a property test |
| Q7 | Output files are private | The database, exports and baseline files are created for the current user only |
| Q8 | Terminal and notification text is safe | Paths, commit subjects and context lines are shown without control characters |

## Quality requirements

| No. | Requirement | Accepted when |
|---|---|---|
| R1 | The same checks on Linux, macOS and Windows | The check workflow is green on the three systems for the final commit |
| R2 | A build without the dashboard | `make build-nogui` and `make test-nogui` pass |
| R3 | No comments in source files, no email address in the repository | The repository tests pass |
| R4 | Dependencies are reviewed | Every dependency is pinned, has an accepted license and appears in the notices; the vulnerability check passes |
