#!/usr/bin/env bats

load helpers

setup() {
  make_sandbox
  make_release_repo
  fake_gh
}

teardown() {
  remove_sandbox
}

verify() {
  "$RELEASE_REPO/scripts/release-verify.sh" "$@"
}

@test "release-verify accepts the tag, the commit and exactly three files" {
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES"
  [ "$status" -eq 0 ]
  [[ "$output" == *"release checks passed"* ]]
  grep -q "release view $RELEASE_TAG" "$GH_LOG"
}

@test "release-verify accepts the monorepo tag form" {
  run verify --tag "$APP_SLUG/$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -eq 0 ]
  [ ! -s "$GH_LOG" ]
}

@test "release-verify rejects a tag with another version" {
  git -C "$RELEASE_REPO" tag v9.9.9
  run verify --tag v9.9.9 --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not match the application version"* ]]
}

@test "release-verify rejects a tag of another project" {
  run verify --tag "other/v$APP_VERSION" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"is not a release tag"* ]]
}

@test "release-verify rejects a tag that points at another commit" {
  printf 'more\n' >"$RELEASE_REPO/extra.txt"
  git -C "$RELEASE_REPO" add -A
  git -C "$RELEASE_REPO" commit --quiet -m "docs: add a file"
  run verify --tag "$RELEASE_TAG" --commit "$(git -C "$RELEASE_REPO" rev-parse HEAD)" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"not at the workflow commit"* ]]
}

@test "release-verify rejects a tag that does not exist" {
  git -C "$RELEASE_REPO" tag -d "$RELEASE_TAG"
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not exist in this checkout"* ]]
}

@test "release-verify never overwrites an existing release" {
  GH_RELEASE_EXISTS=0 run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES"
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
}

@test "release-verify rejects an extra file" {
  printf 'sums\n' >"$RELEASE_FILES/SHA256SUMS"
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"exactly the three release files"* ]]
}

@test "release-verify rejects a missing file" {
  rm "$RELEASE_FILES/$APP_SLUG-windows-x64.exe"
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"exactly the three release files"* ]]
}

@test "release-verify rejects an empty file" {
  : >"$RELEASE_FILES/$APP_SLUG-linux-x64.tar.gz"
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"release file is empty"* ]]
}

@test "release-verify needs the release notes" {
  rm "$RELEASE_REPO/docs/RELEASE_NOTES.md"
  run verify --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --directory "$RELEASE_FILES" --skip-remote
  [ "$status" -ne 0 ]
  [[ "$output" == *"RELEASE_NOTES.md is missing"* ]]
}

@test "release-verify needs every argument" {
  run verify --tag "$RELEASE_TAG"
  [ "$status" -ne 0 ]
  [[ "$output" == *"--commit is required"* ]]
}
