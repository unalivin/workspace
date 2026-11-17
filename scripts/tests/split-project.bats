#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  commit_all "feat(certwatch): create project from template"
  printf 'more\n' >>"$WORKSPACE_ROOT/certwatch/CHANGELOG.md"
  commit_all "docs(certwatch): extend changelog"
}

teardown() {
  remove_workspace
}

@test "split-project prints help" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/split-project.sh"* ]]
}

@test "split-project creates a standalone repository with the project history" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  [ -f "$TEST_ROOT/standalone/README.md" ]
  [ -f "$TEST_ROOT/standalone/cmd/certwatch/metadata.txt" ]
  [ ! -e "$TEST_ROOT/standalone/certwatch" ]
  [ ! -e "$TEST_ROOT/standalone/scripts/projects.tsv" ]
  run git -C "$TEST_ROOT/standalone" log --format=%s
  [ "${lines[0]}" = "ci: restore standalone workflows and repository links" ]
  [ "${lines[1]}" = "docs(certwatch): extend changelog" ]
  [ "${lines[2]}" = "feat(certwatch): create project from template" ]
  [ "${#lines[@]}" -eq 3 ]
  run git -C "$TEST_ROOT/standalone" rev-parse --abbrev-ref HEAD
  [ "$output" = "main" ]
  run git -C "$TEST_ROOT/standalone" status --porcelain
  [ -z "$output" ]
  run git -C "$TEST_ROOT/standalone" remote
  [ -z "$output" ]
}

@test "split-project restores standalone workflows with plain tags" {
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  local ci="$TEST_ROOT/standalone/.github/workflows/ci.yml"
  local release="$TEST_ROOT/standalone/.github/workflows/release.yml"
  [ -f "$ci" ]
  [ -f "$release" ]
  grep -q "working-directory: \." "$ci"
  grep -q "'v\*'" "$release"
  refute grep -q "certwatch/v" "$release"
}

@test "split-project rewrites repository links and tag references" {
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  grep -q "^Repository: https://github.com/unalivin/certwatch$" "$TEST_ROOT/standalone/README.md"
  grep -q "^Release tag: v0.1.0-dev$" "$TEST_ROOT/standalone/README.md"
  refute grep -rq "unalivin/workspace" "$TEST_ROOT/standalone" --exclude-dir=.git
}

@test "split-project leaves the monorepo untouched" {
  local head branches
  head="$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)"
  branches="$(git -C "$WORKSPACE_ROOT" for-each-ref --format='%(refname)')"
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)" = "$head" ]
  [ "$(git -C "$WORKSPACE_ROOT" for-each-ref --format='%(refname)')" = "$branches" ]
  [ -z "$(git -C "$WORKSPACE_ROOT" status --porcelain)" ]
  grep -q "unalivin/workspace" "$WORKSPACE_ROOT/certwatch/README.md"
}

@test "split-project dry run creates nothing" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --dry-run --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: would split certwatch"* ]]
  [ ! -e "$TEST_ROOT/standalone" ]
}

@test "split-project rejects bad input" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh"
  [ "$status" -ne 0 ]
  run "$WORKSPACE_ROOT/scripts/split-project.sh" nope
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown project"* ]]
  run "$WORKSPACE_ROOT/scripts/split-project.sh" netpulse
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not exist"* ]]
  mkdir "$TEST_ROOT/taken"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/taken" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"output already exists"* ]]
  printf 'dirty\n' >>"$WORKSPACE_ROOT/certwatch/README.md"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"uncommitted changes"* ]]
}

@test "split-project keeps the personal owner, holder and pull request template of a personal project" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  [[ "$output" == *"repository: https://github.com/unalivin/certwatch, copyright holder: Unalivin"* ]]
  grep -q 'Module    = "github.com/unalivin/certwatch"' "$TEST_ROOT/standalone/internal/meta/meta.go"
  grep -qx "Copyright (c) 2026 Unalivin" "$TEST_ROOT/standalone/LICENSE"
  cmp "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md" "$TEST_ROOT/standalone/.github/PULL_REQUEST_TEMPLATE.md"
  [ ! -e "$TEST_ROOT/standalone/.github/PULL_REQUEST_TEMPLATE" ]
}

@test "split-project sends a company project to Sonix-Devs with its holder" {
  "$WORKSPACE_ROOT/scripts/new-project.sh" netauto go >/dev/null
  commit_all "feat(netauto): create project from template"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --dry-run --output "$TEST_ROOT/company" netauto
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: destination: Sonix-Devs organization (company)"* ]]
  [[ "$output" == *"https://github.com/Sonix-Devs/netauto"* ]]
  [[ "$output" == *"dry-run: would set and verify the copyright holder Sonix Devs"* ]]
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/company" netauto
  [ "$status" -eq 0 ]
  local out="$TEST_ROOT/company"
  grep -q "^Repository: https://github.com/Sonix-Devs/netauto$" "$out/README.md"
  grep -q 'Module    = "github.com/Sonix-Devs/netauto"' "$out/internal/meta/meta.go"
  grep -q 'AppID     = "io.github.sonix-devs.netauto"' "$out/internal/meta/meta.go"
  grep -q 'Publisher = "Sonix Devs"' "$out/internal/meta/meta.go"
  grep -qx "Copyright (c) 2026 Sonix Devs" "$out/LICENSE"
  grep -qx 'APP_PUBLISHER="Sonix Devs"' "$out/scripts/lib.sh"
  grep -q "The title uses the scope \`netauto\`" "$out/.github/PULL_REQUEST_TEMPLATE.md"
  refute grep -rq "unalivin\|Unalivin" "$out" --exclude-dir=.git
}

@test "split-project --owner overrides the owner and rewrites module path and identifier" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --owner Some-Org --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  grep -q "^Repository: https://github.com/Some-Org/certwatch$" "$TEST_ROOT/standalone/README.md"
  grep -q 'Module    = "github.com/Some-Org/certwatch"' "$TEST_ROOT/standalone/internal/meta/meta.go"
  grep -q 'AppID     = "io.github.some-org.certwatch"' "$TEST_ROOT/standalone/internal/meta/meta.go"
  grep -q 'Publisher = "Unalivin"' "$TEST_ROOT/standalone/internal/meta/meta.go"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --owner "bad owner" --output "$TEST_ROOT/other" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid owner"* ]]
}

@test "split-project sets a stale holder from the registry and refuses what it cannot verify" {
  sed -i.bak 's/Copyright (c) 2026 Unalivin/Copyright (c) 2026 Sonix Devs/' "$WORKSPACE_ROOT/certwatch/LICENSE"
  rm "$WORKSPACE_ROOT/certwatch/LICENSE.bak"
  commit_all "docs(certwatch): wrong holder"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  grep -qx "Copyright (c) 2026 Unalivin" "$TEST_ROOT/standalone/LICENSE"
  sed -i.bak 's/^APP_PUBLISHER=.*/APP_PUBLISHER="Nobody"/' "$WORKSPACE_ROOT/certwatch/scripts/lib.sh"
  rm "$WORKSPACE_ROOT/certwatch/scripts/lib.sh.bak"
  commit_all "build(certwatch): wrong publisher"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/refused" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *'certwatch: scripts/lib.sh does not set APP_PUBLISHER="Unalivin"'* ]]
  [[ "$output" == *"does not name the copyright holder Unalivin everywhere"* ]]
  [ ! -e "$TEST_ROOT/refused" ]
}

@test "split-project refuses a project without its pull request template" {
  git -C "$WORKSPACE_ROOT" rm --quiet certwatch/.github/PULL_REQUEST_TEMPLATE.md
  commit_all "chore(certwatch): drop the template"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"certwatch has no .github/PULL_REQUEST_TEMPLATE.md"* ]]
}
