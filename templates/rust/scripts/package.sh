#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/package.sh [--output <directory>] [--executable <file>] [--dry-run]

Builds the release file of $APP_NAME for this operating system and writes it
to dist/:

  Linux    $APP_SLUG-linux-<arch>.tar.gz    binary, desktop entry, icon, install Makefile, licenses, usage guide
  macOS    $APP_SLUG-macos-<arch>.tar.gz    $APP_NAME.app made by cargo bundle $CARGO_BUNDLE_VERSION, licenses, usage guide
  Windows  $APP_SLUG-windows-<arch>.exe     one executable with the icon and the publisher embedded by the build script

The publisher "$APP_PUBLISHER" and the copyright line of LICENSE are written
into README.txt, the macOS bundle and the Windows version resource.

The archives contain LICENSE, THIRD_PARTY_NOTICES.md, README.txt and USAGE.md.
The Windows executable carries the same texts inside: "$APP_SLUG licenses"
and "$APP_SLUG guide" print them. Nothing is signed or notarized.

  --output      Directory for the result (default: dist)
  --executable  Package this existing binary instead of building one (Linux only)
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

if [ -n "$executable" ]; then
  [ "$os" = "linux" ] || die "--executable is supported on Linux only"
  [ -f "$executable" ] || die "not a file: $executable"
  executable="$(cd "$(dirname "$executable")" && pwd)/$(basename "$executable")"
  [ "$(basename "$executable")" = "$APP_SLUG" ] || die "the binary must be named $APP_SLUG, found $(basename "$executable")"
fi

cd "$PROJECT_ROOT"
case "$output" in
  /* | [A-Za-z]:*) ;;
  *) output="$PROJECT_ROOT/$output" ;;
esac
work="$output/package-work"
name="$APP_SLUG-$os-$arch"
package="$APP_SLUG-app"
copyright="$(grep -m 1 '^Copyright (c) ' LICENSE || true)"
[ -n "$copyright" ] || die "LICENSE has no copyright line"
case "$copyright" in
  *" $APP_PUBLISHER") ;;
  *) die "the copyright line of LICENSE does not name the publisher $APP_PUBLISHER: $copyright" ;;
esac

write_install_makefile() {
  local target="$1"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: write $target"
    return 0
  fi
  cat >"$target" <<INSTALL
PREFIX ?= /usr/local
USER_PREFIX ?= \$(HOME)/.local

.PHONY: install uninstall user-install user-uninstall

install:
	install -Dm755 usr/local/bin/$APP_SLUG "\$(DESTDIR)\$(PREFIX)/bin/$APP_SLUG"
	install -Dm644 usr/local/share/applications/$APP_ID.desktop "\$(DESTDIR)\$(PREFIX)/share/applications/$APP_ID.desktop"
	install -Dm644 usr/local/share/pixmaps/$APP_ID.png "\$(DESTDIR)\$(PREFIX)/share/pixmaps/$APP_ID.png"

uninstall:
	rm -f "\$(DESTDIR)\$(PREFIX)/bin/$APP_SLUG"
	rm -f "\$(DESTDIR)\$(PREFIX)/share/applications/$APP_ID.desktop"
	rm -f "\$(DESTDIR)\$(PREFIX)/share/pixmaps/$APP_ID.png"

user-install:
	install -Dm755 usr/local/bin/$APP_SLUG "\$(USER_PREFIX)/bin/$APP_SLUG"
	install -Dm644 usr/local/share/applications/$APP_ID.desktop "\$(USER_PREFIX)/share/applications/$APP_ID.desktop"
	install -Dm644 usr/local/share/pixmaps/$APP_ID.png "\$(USER_PREFIX)/share/icons/$APP_ID.png"
	sed -i -e "s|^Exec=.*|Exec=\$(USER_PREFIX)/bin/$APP_SLUG|" -e "s|^Icon=.*|Icon=\$(USER_PREFIX)/share/icons/$APP_ID.png|" "\$(USER_PREFIX)/share/applications/$APP_ID.desktop"

user-uninstall:
	rm -f "\$(USER_PREFIX)/bin/$APP_SLUG"
	rm -f "\$(USER_PREFIX)/share/applications/$APP_ID.desktop"
	rm -f "\$(USER_PREFIX)/share/icons/$APP_ID.png"
INSTALL
}

write_desktop_entry() {
  local target="$1"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: write the desktop entry from assets/$APP_SLUG.desktop"
    return 0
  fi
  {
    sed -e "s|^Icon=.*|Icon=$APP_ID|" "assets/$APP_SLUG.desktop"
    printf 'StartupWMClass=%s\n' "$APP_ID"
  } >"$target"
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
Publisher: $APP_PUBLISHER
$copyright
License: MIT, see LICENSE. Third-party components: THIRD_PARTY_NOTICES.md.
Source and bug reports: $APP_REPOSITORY_URL
README
}

add_documents() {
  local target="$1"
  run cp LICENSE THIRD_PARTY_NOTICES.md "$target/"
  run cp docs/USAGE.md "$target/USAGE.md"
}

build_release() {
  run cargo build --locked --release -p "$package"
}

check_bundle_copyright() {
  local plist="$1"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: check the copyright text in $plist"
    return 0
  fi
  [ -f "$plist" ] || die "the application bundle has no Info.plist"
  [ "$(/usr/libexec/PlistBuddy -c "Print :NSHumanReadableCopyright" "$plist" 2>/dev/null)" = "$copyright" ] ||
    die "the application bundle does not carry the copyright text: $copyright"
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

run rm -rf "$work"
run mkdir -p "$work/$name"

case "$os" in
  linux)
    if [ -z "$executable" ]; then
      build_release
      executable="target/release/$APP_SLUG"
    fi
    run mkdir -p "$work/$name/usr/local/bin" "$work/$name/usr/local/share/applications" "$work/$name/usr/local/share/pixmaps"
    run cp "$executable" "$work/$name/usr/local/bin/$APP_SLUG"
    run chmod 755 "$work/$name/usr/local/bin/$APP_SLUG"
    run cp "assets/linux/$APP_SLUG.png" "$work/$name/usr/local/share/pixmaps/$APP_ID.png"
    write_desktop_entry "$work/$name/usr/local/share/applications/$APP_ID.desktop"
    write_install_makefile "$work/$name/Makefile"
    add_documents "$work/$name"
    write_readme "$work/$name/README.txt" "Start: usr/local/bin/$APP_SLUG
Install for your user (no administrator rights): make user-install
Install for everyone: sudo make install"
    run tar -czf "$output/$name.tar.gz" -C "$work" "$name"
    result="$output/$name.tar.gz"
    ;;
  macos)
    if [ "$DRY_RUN" != "1" ]; then
      have_cargo_tool bundle || die "cargo bundle is not installed: run scripts/dev-setup.sh --install"
    fi
    run cargo bundle --release --format osx --package "$package"
    run cp -R "target/release/bundle/osx/$APP_NAME.app" "$work/$name/"
    check_bundle_copyright "$work/$name/$APP_NAME.app/Contents/Info.plist"
    add_documents "$work/$name"
    write_readme "$work/$name/README.txt" "Start: open \"$APP_NAME.app\" (the first time: right-click, Open).
Command line: \"$APP_NAME.app/Contents/MacOS/$APP_SLUG\" --help"
    run tar -czf "$output/$name.tar.gz" -C "$work" "$name"
    result="$output/$name.tar.gz"
    ;;
  windows)
    build_release
    run cp "target/release/$APP_SLUG.exe" "$output/$name.exe"
    check_windows_resource "$output/$name.exe"
    result="$output/$name.exe"
    ;;
esac

run rm -rf "$work"
if [ "$DRY_RUN" != "1" ]; then
  [ -s "$result" ] || die "packaging did not produce $result"
fi
note "packaged $result"
