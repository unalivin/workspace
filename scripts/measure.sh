#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/measure.sh [--run] [--shipped] [--help] <project>

Prints the size and completeness numbers of one project and compares them
with its row in scripts/floors.tsv. Exits 1 when any floor is not met.
Tests are counted in the project and in .workspace/dev-archive/<project>.

  --run      also run scripts/check-all.sh for the project and show the last lines
  --shipped  only check that no test or development file remains in the folder
USAGE
}

run_checks=0
shipped_only=0
project=""
while [ $# -gt 0 ]; do
  case "$1" in
    --help) usage; exit 0 ;;
    --run) run_checks=1 ;;
    --shipped) shipped_only=1 ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *) project="$1" ;;
  esac
  shift
done

[ -n "$project" ] || { usage >&2; exit 2; }
[ -d "$project" ] || { echo "No such project folder: $project" >&2; exit 2; }

if [ "$shipped_only" = 1 ]; then
  leftovers=$(find "$project" \( -name '*_test.go' -o -name '*.bats' -o -name '*_test.cpp' \) -o \
    \( -type d \( -name tests -o -name fuzz -o -name testdata -o -name adr \) \) -o \
    \( -type f \( -name 'TESTING.md' -o -name 'ROADMAP.md' -o -name 'THREAT_MODEL.md' -o -name 'PLATFORM_SUPPORT.md' -o -name 'dev-setup.sh' -o -name 'check.sh' -o -name 'demo.sh' \) \) 2>/dev/null | grep -v '/target/' || true)
  if [ -n "$leftovers" ]; then
    printf 'Development files still in %s:\n%s\n' "$project" "$leftovers"
    exit 1
  fi
  printf 'No development files in %s.\n' "$project"
  exit 0
fi

floors_file="scripts/floors.tsv"
[ -f "$floors_file" ] || { echo "Missing $floors_file" >&2; exit 2; }

row=$(awk -F'\t' -v p="$project" '$1 == p { print }' "$floors_file")
[ -n "$row" ] || { echo "No row for $project in $floors_file" >&2; exit 2; }
IFS=$'\t' read -r _ size_class min_lines min_tests min_screens <<<"$row"

if [ -f "$project/go.mod" ]; then
  language=go
elif [ -f "$project/Cargo.toml" ]; then
  language=rust
elif [ -f "$project/CMakeLists.txt" ]; then
  language=cpp
else
  echo "Cannot detect the language of $project" >&2
  exit 2
fi

test_roots="$project"
[ -d ".workspace/dev-archive/$project" ] && test_roots="$project .workspace/dev-archive/$project"

count_lines() {
  local total=0 n file
  while IFS= read -r file; do
    if [ "$language" = rust ]; then
      n=$(awk '/#\[cfg\(test\)\]/ { exit } NF { c++ } END { print c + 0 }' "$file")
    else
      n=$(awk 'NF { c++ } END { print c + 0 }' "$file")
    fi
    total=$((total + n))
  done
  echo "$total"
}

case "$language" in
  go)
    prod_lines=$(find "$project" -name '*.go' ! -name '*_test.go' ! -path '*/testdata/*' ! -path '*/vendor/*' | count_lines)
    tests=$( (grep -rhE '^func (Test|Fuzz)[A-Za-z0-9_]*\(' $test_roots --include='*_test.go' || true) | wc -l)
    front_dirs="$project/internal $project/cmd"
    ;;
  rust)
    prod_lines=$(find "$project" -name '*.rs' ! -path '*/tests/*' ! -path '*/target/*' ! -path '*/fuzz/*' | count_lines)
    tests=$( (find $test_roots -name '*.rs' ! -path '*/target/*' -print0 | xargs -0 grep -hE '#\[test\]|proptest!' || true) | wc -l)
    front_dirs="$project/crates"
    ;;
  cpp)
    prod_lines=$(find "$project/src" \( -name '*.cpp' -o -name '*.cc' -o -name '*.hpp' -o -name '*.h' \) | count_lines)
    tests=$( (grep -rhE '^(TEST_CASE|TEST)\(' $test_roots --include='*.cpp' || true) | wc -l)
    front_dirs="$project/src"
    ;;
esac

screens_ok=0
screens_file=".workspace/screens/$project.tsv"
if [ -f "$screens_file" ]; then
  while IFS=$'\t' read -r category gui_symbol tui_symbol; do
    [ "$category" = category ] && continue
    [ -n "$category" ] || continue
    # shellcheck disable=SC2086
    if grep -rqF -- "$gui_symbol" $front_dirs 2>/dev/null && grep -rqF -- "$tui_symbol" $front_dirs 2>/dev/null; then
      screens_ok=$((screens_ok + 1))
    fi
  done <"$screens_file"
fi

migrations=$(find "$project/migrations" -name '[0-9][0-9][0-9][0-9]_*.sql' 2>/dev/null | wc -l)
sql_tables=$( (cat "$project"/migrations/*.sql 2>/dev/null || true) | grep -ciE '^create (virtual )?table' || true)

icon_bytes=0
[ -f "$project/assets/icon.svg" ] && icon_bytes=$(wc -c <"$project/assets/icon.svg")
png_count=$(find "$project/assets" -name '*.png' 2>/dev/null | wc -l)
has_ico=$(find "$project/assets" -name '*.ico' 2>/dev/null | wc -l)
has_icns=$(find "$project/assets" -name '*.icns' 2>/dev/null | wc -l)

diagram_count=0
for d in architecture flow data-model; do
  [ -s "$project/docs/$d.svg" ] && diagram_count=$((diagram_count + 1))
done

failures=0
report() {
  local label="$1" actual="$2" need="$3" ok="$4"
  local mark=PASS
  if [ "$ok" != 1 ]; then
    mark=FAIL
    failures=$((failures + 1))
  fi
  printf '%-28s %8s  (need %s)  %s\n' "$label" "$actual" "$need" "$mark"
}

ge() { [ "$1" -ge "$2" ] && echo 1 || echo 0; }

printf 'Project %s, language %s, size class %s\n\n' "$project" "$language" "$size_class"
report "production lines" "$prod_lines" ">= $min_lines" "$(ge "$prod_lines" "$min_lines")"
report "test functions" "$tests" ">= $min_tests" "$(ge "$tests" "$min_tests")"
report "screens in GUI and TUI" "$screens_ok" ">= $min_screens" "$(ge "$screens_ok" "$min_screens")"
report "migrations" "$migrations" ">= 2" "$(ge "$migrations" 2)"
report "SQL tables" "$sql_tables" ">= 8" "$(ge "$sql_tables" 8)"
report "icon.svg bytes" "$icon_bytes" ">= 1500" "$(ge "$icon_bytes" 1500)"
report "icon PNG sizes" "$png_count" ">= 7" "$(ge "$png_count" 7)"
report ".ico and .icns files" "$((has_ico + has_icns))" ">= 2" "$(ge "$((has_ico + has_icns))" 2)"
report "diagrams" "$diagram_count" ">= 3" "$(ge "$diagram_count" 3)"

if [ "$run_checks" = 1 ]; then
  printf '\ncheck-all output (last 15 lines):\n'
  if output=$(scripts/check-all.sh "$project" 2>&1); then
    printf '%s\n' "$output" | tail -15
    printf 'check-all: exit 0\n'
  else
    printf '%s\n' "$output" | tail -15
    printf 'check-all: FAILED\n'
    failures=$((failures + 1))
  fi
fi

printf '\n'
if [ "$failures" -gt 0 ]; then
  printf '%s does not meet the floor (%d failing rows). State stays In progress.\n' "$project" "$failures"
  exit 1
fi
printf '%s meets the floor.\n' "$project"
