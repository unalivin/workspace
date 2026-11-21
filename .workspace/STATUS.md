# Workspace Status

- Date: 2026-11-21
- Last commit: pending initial commit

| Project | Language | Destination | State | Next step |
|---|---|---|---|---|
| logforge-siem | rust | sonix-devs | Complete | Extract Rust template to templates/rust/, then start netauto |
| netauto | go | sonix-devs | In progress | Schema migrations and SQL tests |
| vulntrack | go | sonix-devs | Not started | Waiting on netauto |
| forensicflow | rust | sonix-devs | Not started | Waiting on vulntrack |
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
