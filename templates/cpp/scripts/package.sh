#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/package.sh [--output <directory>] [--dry-run]

Builds the release file of $APP_NAME for this operating system and writes it
to dist/:

  Linux    $APP_SLUG-linux-<arch>.tar.gz    binary, desktop entry, icon, install Makefile, licenses, usage guide
  macOS    $APP_SLUG-macos-<arch>.tar.gz    $APP_NAME.app, licenses, usage guide
  Windows  $APP_SLUG-windows-<arch>.exe     one executable with the icon and the publisher in its version resource

The archives are made by CPack from the install rules in cmake/Package.cmake.
The publisher "$APP_PUBLISHER" and the copyright line of LICENSE are written
into README.txt, the macOS bundle and the Windows version resource.

The archives contain LICENSE, THIRD_PARTY_NOTICES.md, README.txt and USAGE.md.
The Windows executable carries the same texts inside: "$APP_SLUG licenses"
and "$APP_SLUG guide" print them. Nothing is signed or notarized.

  --output   Directory for the result (default: dist)
  --dry-run  Print the commands instead of running them
  --help     Show this help
USAGE
}

output="$PROJECT_ROOT/dist"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      [ "$#" -ge 2 ] || die "--output needs a directory"
      output="$2"
      shift
      ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

os="$(host_os)"
arch="$(host_arch)"
[ "$os" != "unknown" ] || die "unsupported operating system: $(uname -s)"

cd "$PROJECT_ROOT"
case "$output" in
  /* | [A-Za-z]:*) ;;
  *) output="$PROJECT_ROOT/$output" ;;
esac
work="$output/package-work"
name="$APP_SLUG-$os-$arch"
preset="release"
[ -f LICENSE ] || die "LICENSE is missing"
copyright="$(grep -m 1 '^Copyright (c) ' LICENSE || true)"
[ -n "$copyright" ] || die "LICENSE has no copyright line"
case "$copyright" in
  *" $APP_PUBLISHER") ;;
  *) die "the copyright line of LICENSE does not name the publisher $APP_PUBLISHER: $copyright" ;;
esac

check_bundle_copyright() {
  local archive="$1" plist="$work/Info.plist"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: check the copyright text in the Info.plist of $archive"
    return 0
  fi
  tar -xzOf "$archive" "$name/$APP_NAME.app/Contents/Info.plist" >"$plist" || die "the application bundle has no Info.plist"
  [ "$(/usr/libexec/PlistBuddy -c "Print :NSHumanReadableCopyright" "$plist" 2>/dev/null)" = "$copyright" ] ||
    die "the application bundle does not carry the copyright text: $copyright"
  [ "$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$plist" 2>/dev/null)" = "$APP_ID" ] ||
    die "the application bundle does not carry the identifier $APP_ID"
}

check_archive_readme() {
  local archive="$1" readme
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: check that README.txt in $archive names the publisher $APP_PUBLISHER"
    return 0
  fi
  readme="$(tar -xzOf "$archive" "$name/README.txt")" || die "$archive has no README.txt"
  case "$readme" in
    *"Publisher: $APP_PUBLISHER"*"$copyright"*) ;;
    *) die "README.txt in $archive does not name the publisher and the copyright line" ;;
  esac
}

check_archive_layout() {
  local archive="$1" listing entry
  shift
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: check that $archive contains $*"
    return 0
  fi
  listing="$(tar -tzf "$archive")" || die "$archive cannot be read"
  for entry in "$@"; do
    printf '%s\n' "$listing" | grep -Fxq "$name/$entry" || die "$archive does not contain $entry"
  done
}

windows_resource_field() {
  local file="$1" field="$2" path
  path="$(cygpath -w "$file")"
  powershell.exe -NoProfile -NonInteractive -Command "(Get-Item -LiteralPath '$path').VersionInfo.$field" | tr -d '\r'
}

check_windows_resource() {
  local target="$1"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: check that the version resource names the company $APP_PUBLISHER"
    return 0
  fi
  [ "$(windows_resource_field "$target" CompanyName)" = "$APP_PUBLISHER" ] ||
    die "the version resource of $(basename "$target") does not name the company $APP_PUBLISHER"
  [ "$(windows_resource_field "$target" LegalCopyright)" = "$copyright" ] ||
    die "the version resource of $(basename "$target") does not carry the copyright line"
  [ "$(windows_resource_field "$target" ProductName)" = "$APP_NAME" ] ||
    die "the version resource of $(basename "$target") does not name the product $APP_NAME"
}

build_preset "$preset" "$APP_SLUG"
run rm -rf "$work"
run mkdir -p "$work"

case "$os" in
  linux | macos)
    run cpack --config "$(preset_dir "$preset")/CPackConfig.cmake" -C "$(preset_config "$preset")" -G TGZ -B "$work"
    run mv "$work/$name.tar.gz" "$output/$name.tar.gz"
    result="$output/$name.tar.gz"
    check_archive_readme "$result"
    if [ "$os" = "macos" ]; then
      check_archive_layout "$result" LICENSE THIRD_PARTY_NOTICES.md USAGE.md README.txt \
        "$APP_NAME.app/Contents/MacOS/$APP_SLUG" "$APP_NAME.app/Contents/Info.plist" \
        "$APP_NAME.app/Contents/Resources/$APP_SLUG.icns"
      check_bundle_copyright "$result"
    else
      check_archive_layout "$result" LICENSE THIRD_PARTY_NOTICES.md USAGE.md README.txt Makefile \
        "usr/local/bin/$APP_SLUG" "usr/local/share/applications/$APP_ID.desktop" "usr/local/share/pixmaps/$APP_ID.png"
    fi
    ;;
  windows)
    run cp "$(built_program "$preset" "$APP_SLUG")" "$output/$name.exe"
    check_windows_resource "$output/$name.exe"
    result="$output/$name.exe"
    ;;
esac

run rm -rf "$work"
if [ "$DRY_RUN" != "1" ]; then
  [ -s "$result" ] || die "packaging did not produce $result"
fi
note "packaged $result"
