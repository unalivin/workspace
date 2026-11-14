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
