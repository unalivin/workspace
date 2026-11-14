#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
}

teardown() {
  remove_workspace
}

@test "list-projects prints help" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/list-projects.sh"* ]]
}

@test "list-projects prints 25 folders in table order" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --folders
  [ "$status" -eq 0 ]
  [ "${#lines[@]}" -eq 25 ]
  [ "${lines[0]}" = "certwatch" ]
  [ "${lines[14]}" = "secretsentry" ]
  [ "${lines[24]}" = "securebaseline" ]
}

@test "list-projects filters by language" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --folders --language go
  [ "${#lines[@]}" -eq 14 ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --folders --language rust
  [ "${#lines[@]}" -eq 8 ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --folders --language cpp
  [ "${#lines[@]}" -eq 3 ]
}

@test "list-projects markdown has a header and one row per project" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --markdown
  [ "$status" -eq 0 ]
  [ "${#lines[@]}" -eq 27 ]
  [[ "${lines[2]}" == "| 1 | \`certwatch\` | CertWatch | Go | TLS Certificate Monitoring | Not started | - |" ]]
}

@test "list-projects links existing folders and lists them with --existing" {
  mkdir -p "$WORKSPACE_ROOT/certwatch"
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --existing
  [ "$output" = "certwatch" ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --markdown
  [[ "${lines[2]}" == *"[docs](certwatch/docs/README.md)"* ]]
}

@test "list-projects default table and tsv output" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh"
  [ "$status" -eq 0 ]
  [ "${#lines[@]}" -eq 26 ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --tsv
  [ "${lines[22]}" = "23"$'\t'"routescope"$'\t'"RouteScope"$'\t'"cpp"$'\t'"Not started" ]
}

@test "list-projects rejects unknown arguments and languages" {
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --bogus
  [ "$status" -ne 0 ]
  run "$WORKSPACE_ROOT/scripts/list-projects.sh" --language python
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown language"* ]]
}
