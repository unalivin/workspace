#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  make_fake_template go
}

teardown() {
  remove_workspace
}

@test "new-project prints help" {
  run "$WORKSPACE_ROOT/scripts/new-project.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/new-project.sh"* ]]
}

@test "new-project copies the template and fills in metadata" {
  run "$WORKSPACE_ROOT/scripts/new-project.sh" dns-sentinel go
  [ "$status" -eq 0 ]
  local project="$WORKSPACE_ROOT/dns-sentinel"
  [ -f "$project/cmd/dns-sentinel/metadata.txt" ]
  [ ! -e "$project/cmd/templateapp" ]
  grep -qx "app=DNS Sentinel" "$project/cmd/dns-sentinel/metadata.txt"
  grep -qx "slug=dns-sentinel" "$project/cmd/dns-sentinel/metadata.txt"
  grep -qx "prefix=DNS_SENTINEL_UI" "$project/cmd/dns-sentinel/metadata.txt"
  grep -qx "tagline=DNS Change Monitoring" "$project/cmd/dns-sentinel/metadata.txt"
  grep -qx "topics=dns,dns-monitoring,dnssec,domain-monitoring,cybersecurity,defensive-security,golang" "$project/cmd/dns-sentinel/metadata.txt"
  grep -q "^icon=a globe drawn as meridian lines" "$project/cmd/dns-sentinel/metadata.txt"
  grep -q "^# DNS Sentinel$" "$project/README.md"
  grep -q "Local-first DNS change monitor for domains you own" "$project/README.md"
  grep -q "^Folder: dns-sentinel$" "$project/README.md"
  grep -q "^Release tag: dns-sentinel/v0.1.0-dev$" "$project/README.md"
  refute grep -rq "templateapp\|Template App\|TEMPLATE" "$project"
}

@test "new-project updates the registry, the README table and the workflows" {
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --tsv
  [ "${lines[0]}" = "1"$'\t'"certwatch"$'\t'"CertWatch"$'\t'"go"$'\t'"In progress" ]
  grep -q "| 1 | \[\`certwatch\`\](certwatch/) | CertWatch | Go | TLS Certificate Monitoring | In progress |" "$WORKSPACE_ROOT/README.md"
  [ -f "$WORKSPACE_ROOT/.github/workflows/certwatch-ci.yml" ]
  [ -f "$WORKSPACE_ROOT/.github/workflows/certwatch-release.yml" ]
  run "$WORKSPACE_ROOT/scripts/verify-repo.sh" registry files workflows readme
  [ "$status" -eq 0 ]
}

@test "new-project keeps the template unchanged" {
  local before after
  before="$(cd "$WORKSPACE_ROOT/templates/go" && find . -type f -exec cksum {} + | sort)"
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  after="$(cd "$WORKSPACE_ROOT/templates/go" && find . -type f -exec cksum {} + | sort)"
  [ "$before" = "$after" ]
}

@test "new-project skips ignored build output in the template" {
  mkdir -p "$WORKSPACE_ROOT/templates/go/dist"
  printf 'binary\n' >"$WORKSPACE_ROOT/templates/go/dist/artifact"
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  [ ! -e "$WORKSPACE_ROOT/certwatch/dist" ]
}

@test "new-project dry run creates nothing" {
  run "$WORKSPACE_ROOT/scripts/new-project.sh" --dry-run certwatch go
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: would copy templates/go to certwatch"* ]]
  [ ! -e "$WORKSPACE_ROOT/certwatch" ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --tsv
  [[ "${lines[0]}" == *"Not started" ]]
}

@test "new-project rejects bad input" {
  run "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch
  [ "$status" -ne 0 ]
  run "$WORKSPACE_ROOT/scripts/new-project.sh" unknown-tool go
  [ "$status" -ne 0 ]
  [[ "$output" == *"not in scripts/projects.tsv"* ]]
  run "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch rust
  [ "$status" -ne 0 ]
  [[ "$output" == *"registered as a go project"* ]]
  run "$WORKSPACE_ROOT/scripts/new-project.sh" secretsentry rust
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not exist yet"* ]]
  run "$WORKSPACE_ROOT/scripts/new-project.sh" "Bad Name" go
  [ "$status" -ne 0 ]
  run "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch python
  [ "$status" -ne 0 ]
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  run "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
}
