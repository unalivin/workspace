#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/package.sh [--output <directory>] [--executable <file>] [--dry-run]

Builds the release file of $APP_NAME for this operating system with
"fyne package" ($FYNE_TOOLS_VERSION) and writes it to dist/:

  Linux    $APP_SLUG-linux-<arch>.tar.gz    binary, desktop entry, icon, licenses, usage guide
  macOS    $APP_SLUG-macos-<arch>.tar.gz    $APP_NAME.app, licenses, usage guide
  Windows  $APP_SLUG-windows-<arch>.exe     one executable with the icon embedded

The archives contain LICENSE, THIRD_PARTY_NOTICES.md, README.txt and USAGE.md.
The Windows executable carries the same texts inside: "$APP_SLUG licenses"
and "$APP_SLUG guide" print them. Nothing is signed or notarized.

  --output      Directory for the result (default: dist)
  --executable  Package this existing binary instead of building one
                (Linux and macOS; on Windows the tool always rebuilds)
  --dry-run     Print the commands instead of running them
  --help        Show this help
USAGE
}

output="$PROJECT_ROOT/dist"
executable=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      [ "$#" -ge 2 ] || die "--output needs a directory"
      output="$2"
      shift
      ;;
    --executable)
      [ "$#" -ge 2 ] || die "--executable needs a file"
      executable="$2"
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
command -v fyne >/dev/null 2>&1 || [ "$DRY_RUN" = "1" ] || die "fyne is not installed: run scripts/dev-setup.sh --install"

if [ -n "$executable" ]; then
  [ "$os" != "windows" ] || die "--executable is not supported on Windows"
  [ -f "$executable" ] || die "not a file: $executable"
  executable="$(cd "$(dirname "$executable")" && pwd)/$(basename "$executable")"
  [ "$(basename "$executable")" = "$APP_SLUG" ] || die "the binary must be named $APP_SLUG, found $(basename "$executable")"
fi

cd "$PROJECT_ROOT"
case "$output" in
  /* | [A-Za-z]:*) ;;
  *) output="$PROJECT_ROOT/$output" ;;
esac
source_dir="cmd/$APP_SLUG"
icon="$PROJECT_ROOT/assets/icon.png"
tags="${GUI_TAGS// /,}"
work="$output/package-work"
name="$APP_SLUG-$os-$arch"

fyne_package() {
  local arguments=(--target "$1" --src "$source_dir" --name "$APP_NAME" --app-id "$APP_ID"
    --app-version "$APP_NUMERIC_VERSION" --app-build 1 --icon "$icon" --tags "$tags" --release)
  if [ -n "$executable" ]; then
    arguments+=(--executable "$executable")
  fi
  run fyne package "${arguments[@]}"
}

write_desktop_entry() {
  local directory="$1" entry
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: write the desktop entry from assets/$APP_SLUG.desktop"
    return 0
  fi
  entry="$(find "$directory" -type f -name "$APP_ID.desktop" | head -n 1)"
  [ -n "$entry" ] || die "the package does not contain $APP_ID.desktop"
  {
    sed -e "s|^Icon=.*|Icon=$APP_ID|" "assets/$APP_SLUG.desktop"
    printf 'StartupWMClass=%s\n' "$APP_NAME"
  } >"$entry"
}

write_readme() {
  local target="$1" start="$2"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: write $target"
    return 0
  fi
  cat >"$target" <<README
$APP_NAME $APP_VERSION (dev beta)

$start

Run "$APP_SLUG --help" for the commands, or read USAGE.md.
The same guide is inside the application: About, Usage guide.

This is an early development build. It is not signed.
License: MIT, see LICENSE. Third-party components: THIRD_PARTY_NOTICES.md.
Source and bug reports: $APP_REPOSITORY_URL
README
}

add_documents() {
  local target="$1"
  run cp LICENSE THIRD_PARTY_NOTICES.md "$target/"
  run cp docs/USAGE.md "$target/USAGE.md"
}

run rm -rf "$work"
run mkdir -p "$work/$name"

case "$os" in
  linux)
    fyne_package linux
    run tar -xJf "$APP_NAME.tar.xz" -C "$work/$name" --strip-components=1
    run rm -f "$APP_NAME.tar.xz"
    write_desktop_entry "$work/$name"
    add_documents "$work/$name"
    binary_path="usr/local/bin/$APP_SLUG"
    if [ "$DRY_RUN" != "1" ]; then
      binary_path="$(cd "$work/$name" && find usr -type f -name "$APP_SLUG" | head -n 1)"
      [ -n "$binary_path" ] || die "the package does not contain the $APP_SLUG binary"
    fi
    write_readme "$work/$name/README.txt" "Start: $binary_path
Install for your user (no administrator rights): make user-install
Install for everyone: sudo make install"
    run tar -czf "$output/$name.tar.gz" -C "$work" "$name"
    result="$output/$name.tar.gz"
    ;;
  macos)
    fyne_package darwin
    run mv "$APP_NAME.app" "$work/$name/"
    add_documents "$work/$name"
    write_readme "$work/$name/README.txt" "Start: open \"$APP_NAME.app\" (the first time: right-click, Open).
Command line: \"$APP_NAME.app/Contents/MacOS/$APP_SLUG\" --help"
    run tar -czf "$output/$name.tar.gz" -C "$work" "$name"
    result="$output/$name.tar.gz"
    ;;
  windows)
    fyne_package windows
    run mv "$source_dir/$APP_NAME.exe" "$output/$name.exe"
    result="$output/$name.exe"
    ;;
esac

run rm -rf "$work"
if [ "$DRY_RUN" != "1" ]; then
  [ -s "$result" ] || die "packaging did not produce $result"
fi
note "packaged $result"
