#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

osv_url="https://api.osv.dev/v1/query"
accepted_licenses=("MIT" "Zlib" "Apache-2.0" "BSD-2-Clause" "BSD-3-Clause" "BSL-1.0" "ISC" "OFL-1.1" "CC0-1.0" "WTFPL"
  "Unlicense" "Public domain" "MIT OR Unlicense")

usage() {
  cat <<USAGE
Usage: scripts/vuln.sh [--offline] [--dry-run]

Supply chain checks on third_party/components.tsv, the list of everything
that is compiled into $APP_NAME:

  1. every component has a license from the accepted list and, where the
     license asks for it, its text in third_party/licenses
  2. the versions equal the versions pinned in vcpkg.json, and the vcpkg
     baseline equals the commit that scripts/dev-setup.sh installs
  3. the compiled-in SQLite is the listed version
  4. no known vulnerability is recorded for the pinned upstream commit of a
     component in the OSV database ($osv_url)

A finding that does not apply can be accepted with a line "<id><tab><reason>"
in third_party/osv-ignore.tsv.

  --offline  Skip step 4, which is the only one that uses the network
  --dry-run  Print the requests instead of sending them
  --help     Show this help
USAGE
}

offline=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline) offline=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

cd "$PROJECT_ROOT"
table="third_party/components.tsv"
manifest="vcpkg.json"
ignored="third_party/osv-ignore.tsv"
[ -f "$table" ] || die "$table is missing"
[ -f "$manifest" ] || die "$manifest is missing"
problems=0

problem() {
  printf 'FAIL %s\n' "$*" >&2
  problems=$((problems + 1))
}

accepted() {
  local license
  for license in "${accepted_licenses[@]}"; do
    [ "$license" != "$1" ] || return 0
  done
  return 1
}

rows() {
  if grep -q '|' "$table"; then
    die "$table must not contain the character |"
  fi
  tr -d '\r' <"$table" | tr '\t' '|'
}

pinned_version() {
  tr -d '\n' <"$manifest" | sed 's/}/}\n/g' | sed -n "s/.*\"name\": *\"$1\", *\"version\": *\"\\([^\"#]*\\)[^\"]*\".*/\\1/p" | head -n 1
}

is_ignored() {
  [ -f "$ignored" ] || return 1
  awk -F '\t' -v id="$1" 'NR > 1 && $1 == id && $2 != "" { found = 1 } END { exit found ? 0 : 1 }' "$ignored"
}

count=0
while IFS='|' read -r name version license source part_of use license_file port commit; do
  [ "$name" != "name" ] || continue
  [ -n "$name" ] || continue
  count=$((count + 1))
  [ -n "$source" ] || problem "$name: no source"
  accepted "$license" || problem "$name: the license $license is not on the list of accepted licenses"
  if [ -n "$license_file" ]; then
    [ -s "third_party/licenses/$license_file" ] || problem "$name: third_party/licenses/$license_file is missing or empty"
  elif [ "$license" != "CC0-1.0" ] && [ "$license" != "Public domain" ] && [ "$license" != "Unlicense" ]; then
    problem "$name: the license $license needs its text in third_party/licenses"
  fi
  case "$use" in
    app | gui | tests) ;;
    *) problem "$name: use must be app, gui or tests, found $use" ;;
  esac
  if [ -n "$port" ]; then
    pinned="$(pinned_version "$port")"
    [ "$pinned" = "$version" ] || problem "$name: $table lists $version, $manifest pins ${pinned:-nothing} for the port $port"
  fi
  if [ -z "$part_of" ] && [ -z "$commit" ]; then
    problem "$name: no upstream commit to look up"
  fi
done < <(rows)
[ "$count" -gt 0 ] || problem "$table lists no component"

baseline="$(sed -n 's/.*"builtin-baseline": *"\([0-9a-f]*\)".*/\1/p' "$manifest")"
[ "$baseline" = "$VCPKG_COMMIT" ] || problem "$manifest has the baseline ${baseline:-none}, scripts/lib.sh installs vcpkg $VCPKG_COMMIT"

sqlite_listed="$(awk -F '\t' '$1 == "SQLite" { print $2 }' "$table")"
sqlite_compiled="$(sed -n 's/^#define SQLITE_VERSION  *"\([0-9.]*\)".*/\1/p' third_party/sqlite/sqlite3.h | head -n 1)"
[ -n "$sqlite_compiled" ] || problem "third_party/sqlite/sqlite3.h names no version"
[ "$sqlite_listed" = "$sqlite_compiled" ] || problem "SQLite: $table lists ${sqlite_listed:-nothing}, third_party/sqlite is $sqlite_compiled"

note "checked the licenses and pinned versions of $count components"

if [ "$offline" = "1" ]; then
  note "skipped the OSV lookup because --offline was given"
else
  if [ "$DRY_RUN" != "1" ]; then
    have curl || die "curl is not installed: use --offline to run the checks that need no network"
  fi
  while IFS='|' read -r name version _ _ _ _ _ _ commit; do
    [ "$name" != "name" ] || continue
    [ -n "$commit" ] || continue
    if [ "$DRY_RUN" = "1" ]; then
      note "dry-run: POST $osv_url {\"commit\":\"$commit\"} ($name $version)"
      continue
    fi
    answer="$(curl --silent --show-error --fail --max-time 30 --retry 2 -H 'Content-Type: application/json' \
      -d "{\"commit\":\"$commit\"}" "$osv_url")" || die "the OSV database could not be asked about $name: use --offline to skip the lookup"
    found=0
    while IFS= read -r id; do
      [ -n "$id" ] || continue
      if is_ignored "$id"; then
        note "accepted $id for $name $version ($ignored)"
      else
        problem "$name $version: $id (https://osv.dev/vulnerability/$id)"
        found=1
      fi
    done < <(printf '%s' "$answer" | grep -o '"id": *"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' | LC_ALL=C sort -u)
    [ "$found" = "1" ] || note "no known vulnerability: $name $version ($commit)"
  done < <(rows)
fi

if [ "$problems" -gt 0 ]; then
  die "$problems supply chain problem(s) were found"
fi
