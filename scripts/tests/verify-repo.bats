#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  FIXTURES="$REAL_ROOT/scripts/tests/testdata/comments"
}

teardown() {
  remove_workspace
}

verify() {
  run "$WORKSPACE_ROOT/scripts/verify-repo.sh" "$@"
}

scan() {
  run env LC_ALL=C awk -v lang="$1" -f "$WORKSPACE_ROOT/scripts/find-comments.awk" "$2"
}

fixture_language() {
  case "$1" in
    rs) printf 'rust' ;;
    *) printf '%s' "$1" ;;
  esac
}

@test "verify-repo prints help and rejects unknown checks" {
  verify --help
  [ "$status" -eq 0 ]
  verify bogus
  [ "$status" -ne 0 ]
}

@test "verify-repo passes on a consistent workspace" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  verify
  [ "$status" -eq 0 ]
  [[ "$output" == *"ok   registry"* ]]
  [[ "$output" == *"ok   comments"* ]]
}

@test "registry check rejects untruthful statuses and bad rows" {
  mkdir "$WORKSPACE_ROOT/netpulse"
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"netpulse exists but is marked Not started"* ]]
  rmdir "$WORKSPACE_ROOT/netpulse"
  awk -F'\t' -v OFS='\t' '$2 == "certwatch" { $5 = "Complete" } { print }' "$REGISTRY_FILE" >"$REGISTRY_FILE.tmp"
  mv "$REGISTRY_FILE.tmp" "$REGISTRY_FILE"
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"certwatch is marked Complete but its folder does not exist"* ]]
}

@test "registry check requires 25 rows and seven topics" {
  sed -i.bak '$d' "$REGISTRY_FILE"
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"expected 25 projects, found 24"* ]]
  mv "$REGISTRY_FILE.bak" "$REGISTRY_FILE"
  awk -F'\t' -v OFS='\t' '$2 == "certwatch" { $7 = "tls,x509" } { print }' "$REGISTRY_FILE" >"$REGISTRY_FILE.tmp"
  mv "$REGISTRY_FILE.tmp" "$REGISTRY_FILE"
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"certwatch has 2 topics, expected 7"* ]]
}

@test "files check reports missing project files, targets and sections" {
  make_fake_template go
  verify files
  [ "$status" -eq 0 ]
  rm "$WORKSPACE_ROOT/templates/go/docs/THREAT_MODEL.md" "$WORKSPACE_ROOT/templates/go/scripts/demo.sh"
  rm -r "$WORKSPACE_ROOT/templates/go/testdata"
  sed -i.bak '/^race-or-sanitize:/d' "$WORKSPACE_ROOT/templates/go/Makefile"
  sed -i.bak '/^## Security$/d' "$WORKSPACE_ROOT/templates/go/README.md"
  rm "$WORKSPACE_ROOT/templates/go/migrations/0001_init.sql"
  verify files
  [ "$status" -eq 1 ]
  [[ "$output" == *"templates/go: missing file docs/THREAT_MODEL.md"* ]]
  [[ "$output" == *"templates/go: missing file scripts/demo.sh"* ]]
  [[ "$output" == *"templates/go: missing directory testdata"* ]]
  [[ "$output" == *"templates/go: Makefile has no target race-or-sanitize"* ]]
  [[ "$output" == *"templates/go: README.md has no section Security"* ]]
  [[ "$output" == *"templates/go: missing migrations/0001_*.sql"* ]]
}

@test "files check reports missing root files" {
  rm "$WORKSPACE_ROOT/SECURITY.md"
  verify files
  [ "$status" -eq 1 ]
  [[ "$output" == *"root: missing file SECURITY.md"* ]]
}

@test "emails check finds an address in tracked and untracked files" {
  verify emails
  [ "$status" -eq 0 ]
  printf 'contact %s@%s for help\n' "someone" "example.org" >"$WORKSPACE_ROOT/docs/NOTE.md"
  verify emails
  [ "$status" -eq 1 ]
  [[ "$output" == *"email address found at docs/NOTE.md:1"* ]]
  [[ "$output" != *"someone@"* ]]
}

@test "emails check ignores version pins and ignored files" {
  printf 'uses: actions/checkout@v7\ngo install example.test/tool@v1.2.3\nnpm install bats@1.13.0\n' >"$WORKSPACE_ROOT/docs/NOTE.md"
  mkdir -p "$WORKSPACE_ROOT/dist"
  printf '%s@%s\n' "someone" "example.org" >"$WORKSPACE_ROOT/dist/ignored.txt"
  verify emails
  [ "$status" -eq 0 ]
}

@test "comments check accepts clean sources in every language" {
  local language
  for language in go rs cpp sql sh; do
    scan "$(fixture_language "$language")" "$FIXTURES/clean.$language"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
  done
}

@test "comments check flags Go comments and misplaced directives" {
  scan go "$FIXTURES/dirty.go"
  [ "$status" -eq 1 ]
  [ "${#lines[@]}" -eq 4 ]
  [[ "${lines[0]}" == *"dirty.go:3: line comment" ]]
  [[ "${lines[1]}" == *"dirty.go:4: block comment" ]]
  [[ "${lines[2]}" == *"dirty.go:5: line comment" ]]
  [[ "${lines[3]}" == *"dirty.go:6: line comment" ]]
}

@test "comments check flags Rust doc and block comments" {
  scan rust "$FIXTURES/dirty.rs"
  [ "$status" -eq 1 ]
  [ "${#lines[@]}" -eq 3 ]
  [[ "${lines[0]}" == *"dirty.rs:1: line comment" ]]
  [[ "${lines[1]}" == *"dirty.rs:3: line comment" ]]
  [[ "${lines[2]}" == *"dirty.rs:4: block comment" ]]
}

@test "comments check flags C++ comments" {
  scan cpp "$FIXTURES/dirty.cpp"
  [ "$status" -eq 1 ]
  [ "${#lines[@]}" -eq 2 ]
  [[ "${lines[0]}" == *"dirty.cpp:1: line comment" ]]
  [[ "${lines[1]}" == *"dirty.cpp:2: block comment" ]]
}

@test "comments check flags SQL comments" {
  scan sql "$FIXTURES/dirty.sql"
  [ "$status" -eq 1 ]
  [ "${#lines[@]}" -eq 2 ]
  [[ "${lines[0]}" == *"dirty.sql:1: line comment" ]]
  [[ "${lines[1]}" == *"dirty.sql:2: block comment" ]]
}

@test "comments check flags Bash comments outside here-documents" {
  scan sh "$FIXTURES/dirty.sh"
  [ "$status" -eq 1 ]
  [ "${#lines[@]}" -eq 3 ]
  [[ "${lines[0]}" == *"dirty.sh:2: comment" ]]
  [[ "${lines[1]}" == *"dirty.sh:3: comment" ]]
  [[ "${lines[2]}" == *"dirty.sh:7: comment" ]]
}

@test "comments check scans repository sources and skips testdata and third_party" {
  mkdir -p "$WORKSPACE_ROOT/demo/internal" "$WORKSPACE_ROOT/demo/testdata" "$WORKSPACE_ROOT/demo/third_party"
  printf 'package a\n\n// fixture\n' >"$WORKSPACE_ROOT/demo/testdata/fixture.go"
  printf '/* vendored */\n' >"$WORKSPACE_ROOT/demo/third_party/lib.c"
  printf 'package a\n' >"$WORKSPACE_ROOT/demo/internal/a.go"
  verify comments
  [ "$status" -eq 0 ]
  printf 'package a\n\n// explain\n' >"$WORKSPACE_ROOT/demo/internal/a.go"
  printf 'SELECT 1; -- note\n' >"$WORKSPACE_ROOT/demo/internal/q.sql"
  verify comments
  [ "$status" -eq 1 ]
  [[ "$output" == *"source comment: demo/internal/a.go:3: line comment"* ]]
  [[ "$output" == *"source comment: demo/internal/q.sql:1: line comment"* ]]
}

@test "workflows and readme checks detect drift" {
  verify workflows readme
  [ "$status" -eq 0 ]
  printf 'drift: true\n' >>"$WORKSPACE_ROOT/.github/workflows/root-ci.yml"
  verify workflows
  [ "$status" -eq 1 ]
  [[ "$output" == *"workflows are out of date"* ]]
  sed -i.bak 's/| CertWatch |/| Renamed |/' "$WORKSPACE_ROOT/README.md"
  rm "$WORKSPACE_ROOT/README.md.bak"
  verify readme
  [ "$status" -eq 1 ]
  [[ "$output" == *"README.md project table is out of date"* ]]
  "$WORKSPACE_ROOT/scripts/sync-readme.sh"
  verify readme
  [ "$status" -eq 0 ]
}

@test "sync-readme and install-shell-tools print help and support dry run" {
  run "$WORKSPACE_ROOT/scripts/sync-readme.sh" --help
  [ "$status" -eq 0 ]
  run "$WORKSPACE_ROOT/scripts/install-shell-tools.sh" --help
  [ "$status" -eq 0 ]
  run "$WORKSPACE_ROOT/scripts/install-shell-tools.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run:"*"shellcheck-py==0.11.0.1"* ]]
  [[ "$output" == *"bats@1.13.0"* ]]
  sed -i.bak 's/| CertWatch |/| Renamed |/' "$WORKSPACE_ROOT/README.md"
  run "$WORKSPACE_ROOT/scripts/sync-readme.sh" --dry-run
  [ "$status" -eq 0 ]
  grep -q "| Renamed |" "$WORKSPACE_ROOT/README.md"
}

set_registry_field() {
  awk -F'\t' -v OFS='\t' -v folder="$1" -v column="$2" -v value="$3" '$2 == folder { $column = value } { print }' "$REGISTRY_FILE" >"$REGISTRY_FILE.tmp"
  mv "$REGISTRY_FILE.tmp" "$REGISTRY_FILE"
}

@test "registry check requires a destination and the holder that belongs to it" {
  verify registry
  [ "$status" -eq 0 ]
  set_registry_field netauto 10 personal
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"netauto goes to personal, so its copyright holder must be Unalivin"* ]]
  [[ "$output" == *"expected 17 projects for sonix-devs, found 16"* ]]
  [[ "$output" == *"expected 8 projects for personal, found 9"* ]]
  set_registry_field netauto 10 elsewhere
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"netauto has unknown destination \"elsewhere\""* ]]
  set_registry_field netauto 10 sonix-devs
  set_registry_field certwatch 11 "Sonix Devs"
  verify registry
  [ "$status" -eq 1 ]
  [[ "$output" == *"certwatch goes to personal, so its copyright holder must be Unalivin, not \"Sonix Devs\""* ]]
}

@test "destinations check fails when a document disagrees with the registry" {
  verify destinations
  [ "$status" -eq 0 ]
  grep -q "^| \`netauto\` | Go | Sonix-Devs organization (company) |$" "$WORKSPACE_ROOT/README.md"
  grep -q "^| \`certwatch\` | Go | Personal profile (unalivin) |$" "$WORKSPACE_ROOT/docs/WORKSPACE.md"
  grep -q "^17 projects will be published in the GitHub organization" "$WORKSPACE_ROOT/docs/ROADMAP.md"
  sed -i.bak "s/^| \`netauto\` | Go | Sonix-Devs organization (company) |$/| \`netauto\` | Go | Personal profile (unalivin) |/" "$WORKSPACE_ROOT/README.md"
  rm "$WORKSPACE_ROOT/README.md.bak"
  verify destinations
  [ "$status" -eq 1 ]
  [[ "$output" == *"README.md: the destinations table does not match the registry"* ]]
  [[ "$output" == *"the destination table disagrees with the registry"* ]]
  "$WORKSPACE_ROOT/scripts/sync-readme.sh" destinations >/dev/null
  verify destinations
  [ "$status" -eq 0 ]
  set_registry_field certwatch 10 sonix-devs
  verify destinations
  [ "$status" -eq 1 ]
  [[ "$output" == *"docs/WORKSPACE.md: the destinations table does not match the registry"* ]]
  [[ "$output" == *"docs/ROADMAP.md: the destinations table does not match the registry"* ]]
  printf '# document\n' >"$WORKSPACE_ROOT/docs/ROADMAP.md"
  verify destinations
  [ "$status" -eq 1 ]
  [[ "$output" == *"docs/ROADMAP.md has no destinations:start marker"* ]]
}

@test "pull-requests check fails for a missing template and for a differing copy" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  verify pull-requests
  [ "$status" -eq 0 ]
  printf 'changed\n' >>"$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/certwatch.md"
  verify pull-requests
  [ "$status" -eq 1 ]
  [[ "$output" == *"out of date: .github/PULL_REQUEST_TEMPLATE/certwatch.md"* ]]
  [[ "$output" == *"pull request templates are missing or differ"* ]]
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh" >/dev/null
  rm "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md"
  verify pull-requests
  [ "$status" -eq 1 ]
  [[ "$output" == *"missing: certwatch/.github/PULL_REQUEST_TEMPLATE.md"* ]]
  verify files
  [ "$status" -eq 1 ]
  [[ "$output" == *"certwatch: missing file .github/PULL_REQUEST_TEMPLATE.md"* ]]
}

@test "holders check names every file that disagrees with the registry" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  "$WORKSPACE_ROOT/scripts/new-project.sh" netauto go >/dev/null
  verify holders
  [ "$status" -eq 0 ]
  local project="$WORKSPACE_ROOT/netauto"
  sed -i.bak 's/Sonix Devs/Unalivin/' "$project/LICENSE" "$project/README.md" "$project/THIRD_PARTY_NOTICES.md" "$project/scripts/lib.sh" "$project/internal/meta/meta.go"
  find "$project" -name '*.bak' -delete
  verify holders
  [ "$status" -eq 1 ]
  [[ "$output" == *'netauto: LICENSE has no line "Copyright (c) <year> Sonix Devs"'* ]]
  [[ "$output" == *"netauto: the License section of README.md does not name Sonix Devs"* ]]
  [[ "$output" == *"netauto: the header of THIRD_PARTY_NOTICES.md does not name Sonix Devs"* ]]
  [[ "$output" == *'netauto: scripts/lib.sh does not set APP_PUBLISHER="Sonix Devs"'* ]]
  [[ "$output" == *'netauto: no meta source file sets the publisher to "Sonix Devs"'* ]]
  [[ "$output" == *"netauto: LICENSE names Unalivin, but the registry says Sonix Devs"* ]]
  [[ "$output" != *"certwatch:"* ]]
  printf 'Published by Sonix Devs\n' >"$WORKSPACE_ROOT/certwatch/docs/EXTRA.md"
  verify holders
  [[ "$output" == *"certwatch: docs/EXTRA.md names Sonix Devs, but the registry says Unalivin"* ]]
}

@test "holders check covers the templates" {
  make_fake_template go
  verify holders
  [ "$status" -eq 0 ]
  sed -i.bak 's/Unalivin/Somebody Else/' "$WORKSPACE_ROOT/templates/go/LICENSE"
  rm "$WORKSPACE_ROOT/templates/go/LICENSE.bak"
  verify holders
  [ "$status" -eq 1 ]
  [[ "$output" == *'templates/go: LICENSE has no line "Copyright (c) <year> Unalivin"'* ]]
}
