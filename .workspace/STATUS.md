# Workspace Status

- Date: 2026-11-21
- Last commit: see `git log -1` on the active branch
- LogForge-SIEM current-revision CI: pending; the format/audit helper execute permission was fixed; measure recorded in the project audit.

| Project | Language | Destination | State | Next step |
|---|---|---|---|---|
| logforge-siem | rust | sonix-devs | In progress | Measure PASS: 8521 lines, 188 tests, 11 screens, 8 migrations, 20 SQL tables; comment scan empty; CI pending on current source; step 1 evidence pending, step 3 in progress |
| netauto | go | sonix-devs | In progress | production lines, tests, screens, migrations, icon assets, diagrams below floors; implement and retest |
| vulntrack | go | sonix-devs | In progress | production lines, tests, screens, migrations, tables, icon assets, diagrams below floors; implement and retest |
| forensicflow | rust | sonix-devs | Not started | Create requirements, acceptance criteria, and threat model |
| ipam-pro | go | sonix-devs | Not started | Waiting on forensicflow |
| securebaseline | cpp | sonix-devs | Not started | Waiting on ipam-pro |
| netgraph | cpp | sonix-devs | Not started | Waiting on securebaseline |
| secops-copilot | go | sonix-devs | Not started | Waiting on netgraph |
| cloudfence | go | sonix-devs | Not started | Waiting on secops-copilot |
| incidentflow | go | sonix-devs | Not started | Waiting on cloudfence |
| fleetops | go | sonix-devs | Not started | Waiting on incidentflow |
| iac-sentinel | rust | sonix-devs | Not started | Waiting on fleetops |
| kubeguard | go | sonix-devs | Not started | Waiting on iac-sentinel |
| netpulse | go | sonix-devs | Not started | Waiting on kubeguard |
| sigmalab | rust | sonix-devs | Not started | Waiting on netpulse |
| sbom-navigator | rust | sonix-devs | Not started | Waiting on sigmalab |
| pipelineshield | go | sonix-devs | Not started | Waiting on sbom-navigator |
| routescope | cpp | personal | Not started | Waiting on pipelineshield |
| threatatlas | go | personal | Not started | Waiting on routescope |
| portwatch | go | personal | Not started | Waiting on threatatlas |
| netconfig-auditor | rust | personal | Not started | Waiting on portwatch |
| secretsentry | rust | personal | Not started | Waiting on netconfig-auditor |
| phishlens | rust | personal | Not started | Waiting on secretsentry |
| certwatch | go | personal | Not started | Waiting on phishlens |
| dns-sentinel | go | personal | Not started | Waiting on certwatch |
