#!/usr/bin/env bats

load helpers

setup() {
  make_sandbox
}

teardown() {
  remove_sandbox
}

copy_build_files() {
  copy_project_scripts "$1"
  cp "$REAL_PROJECT/CMakePresets.json" "$REAL_PROJECT/vcpkg.json" "$1/"
  mkdir -p "$1/fuzz/targets" "$1/third_party"
  cp "$REAL_PROJECT"/fuzz/targets/*.cpp "$1/fuzz/targets/"
  cp "$REAL_PROJECT/third_party/components.tsv" "$1/third_party/"
}

@test "every script prints usage with --help" {
  for script in "$REAL_PROJECT"/scripts/*.sh; do
    [ "$(basename "$script")" != "lib.sh" ] || continue
    run "$script" --help
    [ "$status" -eq 0 ]
    [[ "$output" == Usage:* ]]
  done
}

@test "every script starts with the bash shebang and strict mode" {
  for script in "$REAL_PROJECT"/scripts/*.sh; do
    [ "$(sed -n 1p "$script")" = "#!/usr/bin/env bash" ]
    [ "$(sed -n 2p "$script")" = "set -euo pipefail" ]
    [ -x "$script" ]
  done
}

@test "every script rejects an unknown option" {
  for name in build check demo dev-setup format fuzz lint package publish-github release-verify sanitize vuln; do
    run "$REAL_PROJECT/scripts/$name.sh" --no-such-option
    [ "$status" -ne 0 ]
    [[ "$output" == *"unknown argument"* ]]
  done
}

@test "every required make target has a script or a recipe" {
  for target in fmt fmt-check lint test race-or-sanitize build build-nogui check clean package demo fuzz vuln; do
    grep -Eq "^${target}:" "$REAL_PROJECT/Makefile"
  done
}

@test "build --dry-run configures the preset, builds and runs the tests" {
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/build.sh" nogui --test --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: cmake --preset nogui"* ]]
  [[ "$output" == *"dry-run: cmake --build build/nogui "* ]]
  [[ "$output" == *"dry-run: ctest "* ]]
  [ ! -e "$SANDBOX/project/build" ]
}

@test "build rejects a preset that does not exist" {
  run "$REAL_PROJECT/scripts/build.sh" no-such-preset --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"preset"* ]]
}

@test "every preset the scripts use is defined" {
  for preset in dev nogui release release-nogui asan tsan checked fuzz; do
    grep -q "\"name\": \"$preset\"" "$REAL_PROJECT/CMakePresets.json"
  done
}

@test "package --dry-run prints the release build and creates nothing" {
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/package.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: cmake --preset release"* ]]
  [[ "$output" == *"--target $(project_value APP_SLUG)"* ]]
  [[ "$output" == *"packaged "* ]]
  if [ "$(uname -s)" = "Linux" ]; then
    [[ "$output" == *"cpack --config build/release/CPackConfig.cmake"* ]]
    [[ "$output" == *"contains LICENSE THIRD_PARTY_NOTICES.md USAGE.md README.txt Makefile usr/local/bin/$(project_value APP_SLUG) "* ]]
  fi
  [ ! -e "$SANDBOX/project/dist" ]
  [ ! -e "$SANDBOX/project/build" ]
}

@test "package refuses a license whose copyright line names somebody else" {
  copy_build_files "$SANDBOX/project"
  sed -i.bak 's/^Copyright (c) \([0-9]*\) .*/Copyright (c) \1 Somebody Else/' "$SANDBOX/project/LICENSE"
  run "$SANDBOX/project/scripts/package.sh" --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not name the publisher $(project_value APP_PUBLISHER)"* ]]
  rm "$SANDBOX/project/LICENSE"
  run "$SANDBOX/project/scripts/package.sh" --dry-run
  [ "$status" -ne 0 ]
}

@test "demo --dry-run uses a scratch database and config directory" {
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/demo.sh" --dry-run --ui none
  [ "$status" -eq 0 ]
  [[ "$output" == *"demo load"* ]]
  [[ "$output" == *"db init"* ]]
  [[ "$output" != *"--ui"* ]]
  [ ! -e "$SANDBOX/project/dist" ]
}

@test "demo rejects an unknown front end" {
  run "$REAL_PROJECT/scripts/demo.sh" --dry-run --ui web
  [ "$status" -ne 0 ]
  [[ "$output" == *"gui, tui or none"* ]]
}

@test "check --dry-run lists the suite without running it" {
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/check.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"make --no-print-directory test-nogui"* ]]
  [[ "$output" == *"make --no-print-directory race-or-sanitize"* ]]
  [[ "$output" == *"make --no-print-directory notices-check"* ]]
  [[ "$output" == *"failed:  0"* ]]
}

@test "fuzz --list names every fuzz target" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --list
  [ "$status" -eq 0 ]
  [[ "$output" == *"settings_normalize"* ]]
  count="$(find "$REAL_PROJECT/fuzz/targets" -name '*.cpp' | wc -l)"
  [ "${#lines[@]}" -eq "$count" ]
  for name in "${lines[@]}"; do
    [ -d "$REAL_PROJECT/fuzz/seeds/$name" ]
    [ -n "$(find "$REAL_PROJECT/fuzz/seeds/$name" -type f | head -n 1)" ]
  done
}

@test "fuzz --dry-run prints one libFuzzer command per target" {
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/fuzz.sh" --dry-run --time 2m
  [ "$status" -eq 0 ]
  [[ "$output" == *"fuzz_settings_normalize"*" -max_total_time=120 "*"fuzz/corpus/settings_normalize fuzz/seeds/settings_normalize"* ]]
  count="$(find "$REAL_PROJECT/fuzz/targets" -name '*.cpp' | wc -l)"
  [ "$(printf '%s\n' "$output" | grep -c -- '-max_total_time=120')" -eq "$count" ]
  [ ! -e "$SANDBOX/project/fuzz/corpus" ]
}

@test "fuzz rejects a duration it cannot read" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --dry-run --time soon
  [ "$status" -ne 0 ]
  [[ "$output" == *"5s or 2m"* ]]
}

@test "sanitize --dry-run tests the no-GUI build under both sanitizers" {
  [ "$(uname -s)" != "Linux" ] && [ "$(uname -s)" != "Darwin" ] && skip "the sanitizers run on Linux and macOS"
  copy_build_files "$SANDBOX/project"
  run "$SANDBOX/project/scripts/sanitize.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"cmake --preset asan"* ]]
  [[ "$output" == *"cmake --preset tsan"* ]]
  [[ "$output" == *"ctest "* ]]
  [[ "$output" != *"--preset dev"* ]]
  run "$SANDBOX/project/scripts/sanitize.sh" --dry-run --only thread
  [ "$status" -eq 0 ]
  [[ "$output" != *"--preset asan"* ]]
  run "$SANDBOX/project/scripts/sanitize.sh" --dry-run --only memory
  [ "$status" -ne 0 ]
}

@test "the sanitizer presets build without the dashboard" {
  for preset in asan tsan checked fuzz; do
    block="$(sed -n "/\"name\": \"$preset\",/,/}/p" "$REAL_PROJECT/CMakePresets.json" | sed -n '1,/^    }/p')"
    [[ "$block" == *'"PROJECT_GUI": "OFF"'* ]]
  done
}

@test "vuln --offline accepts the committed component list" {
  run "$REAL_PROJECT/scripts/vuln.sh" --offline
  [ "$status" -eq 0 ]
  [[ "$output" == *"checked the licenses and pinned versions of "* ]]
  [[ "$output" == *"skipped the OSV lookup"* ]]
}

@test "vuln --dry-run prints one lookup per pinned commit and sends nothing" {
  run "$REAL_PROJECT/scripts/vuln.sh" --dry-run
  [ "$status" -eq 0 ]
  commits="$(awk -F '\t' 'NR > 1 && $9 != "" { count++ } END { print count }' "$REAL_PROJECT/third_party/components.tsv")"
  [ "$commits" -ge 5 ]
  [ "$(printf '%s\n' "$output" | grep -c '^dry-run: POST ')" -eq "$commits" ]
}

@test "vuln refuses a license that is not accepted, a version that is not pinned and a missing commit" {
  copy_build_files "$SANDBOX/project"
  mkdir -p "$SANDBOX/project/third_party/licenses" "$SANDBOX/project/third_party/sqlite"
  cp "$REAL_PROJECT"/third_party/licenses/*.txt "$SANDBOX/project/third_party/licenses/"
  grep -m 1 '^#define SQLITE_VERSION ' "$REAL_PROJECT/third_party/sqlite/sqlite3.h" >"$SANDBOX/project/third_party/sqlite/sqlite3.h"
  table="$SANDBOX/project/third_party/components.tsv"
  run "$SANDBOX/project/scripts/vuln.sh" --offline
  [ "$status" -eq 0 ]
  cp "$table" "$SANDBOX/table"
  awk -F '\t' -v OFS='\t' '$1 == "FTXUI" { $3 = "GPL-3.0-only" } { print }' "$SANDBOX/table" >"$table"
  run "$SANDBOX/project/scripts/vuln.sh" --offline
  [ "$status" -ne 0 ]
  [[ "$output" == *"FTXUI: the license GPL-3.0-only is not on the list of accepted licenses"* ]]
  awk -F '\t' -v OFS='\t' '$1 == "FTXUI" { $2 = "9.9.9" } { print }' "$SANDBOX/table" >"$table"
  run "$SANDBOX/project/scripts/vuln.sh" --offline
  [ "$status" -ne 0 ]
  [[ "$output" == *"FTXUI: third_party/components.tsv lists 9.9.9"* ]]
  awk -F '\t' -v OFS='\t' '$1 == "FTXUI" { $9 = "" } { print }' "$SANDBOX/table" >"$table"
  run "$SANDBOX/project/scripts/vuln.sh" --offline
  [ "$status" -ne 0 ]
  [[ "$output" == *"FTXUI: no upstream commit to look up"* ]]
  awk -F '\t' -v OFS='\t' '$1 == "SQLite" { $2 = "3.0.0" } { print }' "$SANDBOX/table" >"$table"
  run "$SANDBOX/project/scripts/vuln.sh" --offline
  [ "$status" -ne 0 ]
  [[ "$output" == *"SQLite: third_party/components.tsv lists 3.0.0"* ]]
}

@test "the accepted licenses are the same in the script and in the generator" {
  from_script="$(sed -n '/^accepted_licenses=(/,/)$/p' "$REAL_PROJECT/scripts/vuln.sh" | grep -o '"[^"]*"' | LC_ALL=C sort)"
  from_generator="$(sed -n '/allowed_licenses() {/,/^}/p' "$REAL_PROJECT/tools/notices.cpp" | grep -o '"[^"]*"' | LC_ALL=C sort)"
  [ -n "$from_script" ]
  [ "$from_script" = "$from_generator" ]
  while IFS= read -r license; do
    grep -Fq "\`${license//\"/}\`" "$REAL_PROJECT/docs/LICENSES.md"
  done <<<"$from_script"
}

@test "format and lint --dry-run name the pinned tools" {
  copy_build_files "$SANDBOX/project"
  mkdir -p "$SANDBOX/project/src"
  printf 'int main() {\n  return 0;\n}\n' >"$SANDBOX/project/src/main.cpp"
  run "$SANDBOX/project/scripts/format.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"clang-format --dry-run --Werror"*"src/main.cpp"* ]]
  run "$SANDBOX/project/scripts/format.sh" --write --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"clang-format -i"* ]]
  LINT_TIDY=0 run "$SANDBOX/project/scripts/lint.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"cppcheck "*"--error-exitcode=1"* ]]
}

@test "ci-step runs a command and passes its output through" {
  run "$REAL_PROJECT/scripts/ci-step.sh" --command "say hello" printf 'hello\n'
  [ "$status" -eq 0 ]
  [[ "$output" == *"==> say hello"* ]]
  [[ "$output" == *"hello"* ]]
}

@test "ci-step reports a failing command with its exit status and an annotation" {
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "break things" bash -c 'printf "first line\nsecond 100%% line\n"; exit 7'
  [ "$status" -eq 7 ]
  [[ "$output" == *"step failed: break things (exit 7)"* ]]
  [[ "$output" == *"::error title="*"break things failed"*"::first line%0Asecond 100%25 line"* ]]
}

@test "ci-step puts the failing test before the last lines of a long log" {
  {
    printf '[doctest] doctest version is "2.5.3"\n'
    printf 'tests/core/widget_test.cpp:12:\n'
    printf 'TEST CASE:  a widget adds\n\n'
    printf 'tests/core/widget_test.cpp:14: ERROR: CHECK( got == 4 ) is NOT correct!\n'
    printf '  values: CHECK( 3 == 4 )\n\n'
    for number in $(seq 1 40); do
      printf 'progress line %s\n' "$number"
    done
  } >"$SANDBOX/long.log"
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "long test" bash -c "cat '$SANDBOX/long.log'; exit 1"
  [ "$status" -eq 1 ]
  annotation="${output##*::error}"
  [[ "$annotation" == *"widget_test.cpp:14: ERROR: CHECK( got == 4 )"*"values: CHECK( 3 == 4 )"*"...%0A"*"progress line 40"* ]]
  [[ "$annotation" != *"progress line 20%0A"* ]]
}

@test "ci-step shows a compiler error with its location" {
  for message in "src/app/main.cpp:7:9: error: use of undeclared identifier 'value'" \
    'src\app\main.cpp(7,9): error C2065: value: undeclared identifier' \
    'main.obj : error LNK2019: unresolved external symbol run' \
    'src/core/store.cpp:40: warning: Possible null pointer dereference [nullPointer]' \
    'CMake Error at CMakeLists.txt:12 (find_package):' \
    'src/core/text.cpp:9:3: runtime error: signed integer overflow'; do
    {
      printf '[1/9] Building CXX object main.cpp.o\n'
      printf '%s\n' "$message"
      for number in $(seq 1 40); do
        printf 'note line %s\n' "$number"
      done
    } >"$SANDBOX/build.log"
    GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "build" bash -c "cat '$SANDBOX/build.log'; exit 1"
    [ "$status" -eq 1 ]
    annotation="${output##*::error}"
    [[ "$annotation" == *"build failed"*"::${message}%0A"* ]]
  done
}

@test "ci-step prints no annotation outside GitHub Actions" {
  GITHUB_ACTIONS="" run "$REAL_PROJECT/scripts/ci-step.sh" --command "break things" false
  [ "$status" -eq 1 ]
  [[ "$output" != *"::error"* ]]
}

@test "ci-step without arguments is a usage error" {
  run "$REAL_PROJECT/scripts/ci-step.sh"
  [ "$status" -eq 2 ]
}

@test "dev-setup --install --dry-run prints the pinned tool versions" {
  SETUP_ALL_TOOLS=1 run "$REAL_PROJECT/scripts/dev-setup.sh" --install --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"cmake==$(project_value CMAKE_VERSION)"* ]]
  [[ "$output" == *"ninja==$(project_value NINJA_VERSION)"* ]]
  [[ "$output" == *"clang-format==$(project_value CLANG_FORMAT_VERSION)"* ]]
  [[ "$output" == *"clang-tidy==$(project_value CLANG_TIDY_VERSION)"* ]]
  [[ "$output" == *"cppcheck==$(project_value CPPCHECK_WHEEL_VERSION)"* ]]
  [[ "$output" == *"fetch --quiet --depth 1 $(project_value VCPKG_REPOSITORY_URL) $(project_value VCPKG_COMMIT)"* ]]
}

@test "the script constants match the application metadata" {
  meta="$REAL_PROJECT/src/core/meta.hpp"
  constant() {
    sed -n "s/^inline constexpr std::string_view $1 = \"\\(.*\\)\";\$/\\1/p" "$meta"
  }
  build_value() {
    sed -n "s/^set($1 \"\\(.*\\)\")\$/\\1/p" "$REAL_PROJECT/CMakeLists.txt"
  }
  [ "$(project_value APP_SLUG)" = "$(constant slug)" ]
  [ "$(project_value APP_NAME)" = "$(constant app_name)" ]
  [ "$(project_value APP_ID)" = "$(constant app_id)" ]
  [ "$(project_value APP_REPOSITORY_URL)" = "$(constant repository_url)" ]
  [ "$(project_value APP_SLUG)" = "$(build_value APP_SLUG)" ]
  [ "$(project_value APP_NAME)" = "$(build_value APP_NAME)" ]
  [ "$(project_value APP_ID)" = "$(build_value APP_ID)" ]
  [ "$(project_value APP_REPOSITORY_URL)" = "$(build_value APP_REPOSITORY_URL)" ]
  [ "$(project_value APP_VERSION)" = "$(tr -d '[:space:]' <"$REAL_PROJECT/VERSION")" ]
  [ "$(project_value APP_NUMERIC_VERSION)" = "$(project_value APP_VERSION | sed 's/-.*//')" ]
  [ "$(project_value VCPKG_COMMIT)" = "$(sed -n 's/.*"builtin-baseline": *"\([0-9a-f]*\)".*/\1/p' "$REAL_PROJECT/vcpkg.json")" ]
  grep -q "^cmake_minimum_required(VERSION $(project_value CMAKE_MINIMUM_VERSION))$" "$REAL_PROJECT/CMakeLists.txt"
  grep -q "\"name\": \"$(project_value APP_SLUG)\"" "$REAL_PROJECT/vcpkg.json"
  publisher="$(project_value APP_PUBLISHER)"
  [ -n "$publisher" ]
  [ "$publisher" = "$(constant publisher)" ]
  [ "$publisher" = "$(build_value APP_PUBLISHER)" ]
  copyright="$(grep -m 1 '^Copyright (c) ' "$REAL_PROJECT/LICENSE")"
  [[ "$copyright" =~ ^Copyright\ \(c\)\ [0-9]{4}(-[0-9]{4})?\ ${publisher}$ ]]
  grep -Fq '@APP_COPYRIGHT@' "$REAL_PROJECT/packaging/Info.plist.in"
  grep -Fq '@APP_COPYRIGHT@' "$REAL_PROJECT/packaging/app.rc.in"
  grep -Fq '@APP_PUBLISHER@' "$REAL_PROJECT/packaging/app.rc.in"
  grep -Fq 'Publisher: @APP_PUBLISHER@' "$REAL_PROJECT/packaging/README.txt.in"
  head -n 8 "$REAL_PROJECT/THIRD_PARTY_NOTICES.md" | grep -Fq "published by $publisher under"
  sed -n '/^## License$/,$p' "$REAL_PROJECT/README.md" | grep -Fq "$publisher"
}

@test "the environment example lists only variables the documentation explains" {
  while IFS='=' read -r name _; do
    [ -n "$name" ] || continue
    grep -q "$name" "$REAL_PROJECT/docs/COMMANDS.md"
  done <"$REAL_PROJECT/.env.example"
}

@test "the install Makefile of the Linux archive installs into a user prefix and removes itself again" {
  [ "$(uname -s)" = "Linux" ] || skip "the archive layout under test is the Linux one"
  slug="$(project_value APP_SLUG)"
  id="$(project_value APP_ID)"
  unpacked="$SANDBOX/unpacked"
  mkdir -p "$unpacked/usr/local/bin" "$unpacked/usr/local/share/applications" "$unpacked/usr/local/share/pixmaps"
  sed -e "s/@APP_SLUG@/$slug/g" -e "s/@APP_ID@/$id/g" "$REAL_PROJECT/packaging/install.mk.in" >"$unpacked/Makefile"
  refute grep -q '@' "$unpacked/Makefile"
  printf '#!/bin/sh\nexit 0\n' >"$unpacked/usr/local/bin/$slug"
  {
    grep -v '^Icon=' "$REAL_PROJECT/assets/$slug.desktop"
    printf 'Icon=%s\n' "$id"
  } >"$unpacked/usr/local/share/applications/$id.desktop"
  cp "$REAL_PROJECT/assets/linux/$slug.png" "$unpacked/usr/local/share/pixmaps/$id.png"
  run make -C "$unpacked" user-install USER_PREFIX="$SANDBOX/prefix"
  [ "$status" -eq 0 ]
  [ -x "$SANDBOX/prefix/bin/$slug" ]
  grep -q "^Exec=$SANDBOX/prefix/bin/$slug$" "$SANDBOX/prefix/share/applications/$id.desktop"
  grep -q "^Icon=$SANDBOX/prefix/share/icons/$id.png$" "$SANDBOX/prefix/share/applications/$id.desktop"
  [ -s "$SANDBOX/prefix/share/icons/$id.png" ]
  run make -C "$unpacked" user-uninstall USER_PREFIX="$SANDBOX/prefix"
  [ "$status" -eq 0 ]
  [ -z "$(find "$SANDBOX/prefix" -type f)" ]
  run make -C "$unpacked" install DESTDIR="$SANDBOX/stage" PREFIX=/opt/app
  [ "$status" -eq 0 ]
  [ -x "$SANDBOX/stage/opt/app/bin/$slug" ]
  [ -s "$SANDBOX/stage/opt/app/share/pixmaps/$id.png" ]
  run make -C "$unpacked" uninstall DESTDIR="$SANDBOX/stage" PREFIX=/opt/app
  [ "$status" -eq 0 ]
  [ -z "$(find "$SANDBOX/stage" -type f)" ]
}
