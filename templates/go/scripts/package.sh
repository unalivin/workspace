#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/package.sh [--output <directory>] [--executable <file>] [--dry-run]

Builds the release file of $APP_NAME for this operating system and writes
it to dist/. Linux and macOS use "fyne package" ($FYNE_TOOLS_VERSION); Windows
uses "go build" with a version resource from goversioninfo ($GOVERSIONINFO_VERSION).

  Linux    $APP_SLUG-linux-<arch>.tar.gz    binary, desktop entry, icon, licenses, usage guide
  macOS    $APP_SLUG-macos-<arch>.tar.gz    $APP_NAME.app, licenses, usage guide
  Windows  $APP_SLUG-windows-<arch>.exe     one executable with the icon and the publisher embedded

The publisher "$APP_PUBLISHER" and the copyright line of LICENSE are written
into README.txt, the macOS bundle and the Windows version resource.

The archives contain LICENSE, THIRD_PARTY_NOTICES.md, README.txt and USAGE.md.
The Windows executable carries the same texts inside: "$APP_SLUG licenses"
and "$APP_SLUG guide" print them. Nothing is signed or notarized.

  --output      Directory for the result (default: dist)
  --executable  Package this existing binary instead of building one
                (Linux and macOS; Windows always builds)
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
if [ "$os" != "windows" ] && [ "$DRY_RUN" != "1" ]; then
  command -v fyne >/dev/null 2>&1 || die "fyne is not installed: run scripts/dev-setup.sh --install"
fi

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
copyright="$(grep -m 1 '^Copyright (c) ' LICENSE || true)"
[ -n "$copyright" ] || die "LICENSE has no copyright line"
case "$copyright" in
  *" $APP_PUBLISHER") ;;
  *) die "the copyright line of LICENSE does not name the publisher $APP_PUBLISHER: $copyright" ;;
esac

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

write_bundle_copyright() {
  local plist="$1"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: write the copyright text into $plist"
    return 0
  fi
  [ -f "$plist" ] || die "the application bundle has no Info.plist"
  /usr/libexec/PlistBuddy -c "Delete :NSHumanReadableCopyright" "$plist" >/dev/null 2>&1 || true
  /usr/libexec/PlistBuddy -c "Add :NSHumanReadableCopyright string $copyright" "$plist"
  [ "$(/usr/libexec/PlistBuddy -c "Print :NSHumanReadableCopyright" "$plist")" = "$copyright" ] ||
    die "the application bundle does not carry the copyright text"
}

windows_resource_field() {
  local file="$1" field="$2" path
  path="$(cygpath -w "$file")"
  powershell.exe -NoProfile -NonInteractive -Command "(Get-Item -LiteralPath '$path').VersionInfo.$field" | tr -d '\r'
}

windows_package() {
  local target="$1" goarch resource arm=false major minor patch
  goarch="$(go env GOARCH 2>/dev/null || printf 'amd64')"
  resource="$source_dir/resource_windows_$goarch.syso"
  if [ "$goarch" = "arm64" ]; then
    arm=true
  fi
  IFS=. read -r major minor patch <<<"$APP_NUMERIC_VERSION"
  if [ "$DRY_RUN" != "1" ]; then
    printf '{}\n' >"$work/versioninfo.json"
    cat >"$work/$APP_SLUG.manifest" <<MANIFEST
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<assemblyIdentity version="$APP_NUMERIC_VERSION.1" processorArchitecture="*" name="$APP_ID" type="win32"/>
</assembly>
MANIFEST
  fi
  run go run "github.com/josephspurrier/goversioninfo/cmd/goversioninfo@$GOVERSIONINFO_VERSION" \
    -64=true -arm="$arm" -o "$resource" -icon assets/icon.ico -manifest "$work/$APP_SLUG.manifest" \
    -company "$APP_PUBLISHER" -copyright "$copyright" -product-name "$APP_NAME" -description "$APP_NAME" \
    -internal-name "$APP_SLUG" -original-name "$(basename "$target")" \
    -file-version "$APP_NUMERIC_VERSION.1" -product-version "$APP_VERSION" \
    -ver-major "$major" -ver-minor "$minor" -ver-patch "$patch" -ver-build 1 \
    -product-ver-major "$major" -product-ver-minor "$minor" -product-ver-patch "$patch" -product-ver-build 1 \
    -translation 1033 -charset 1200 "$work/versioninfo.json"
  run env CGO_ENABLED=1 go build -trimpath -ldflags "-s -w -H=windowsgui" -tags "$tags,release" -o "$target" "./$source_dir"
  run rm -f "$resource"
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
    write_bundle_copyright "$work/$name/$APP_NAME.app/Contents/Info.plist"
    add_documents "$work/$name"
    write_readme "$work/$name/README.txt" "Start: open \"$APP_NAME.app\" (the first time: right-click, Open).
Command line: \"$APP_NAME.app/Contents/MacOS/$APP_SLUG\" --help"
    run tar -czf "$output/$name.tar.gz" -C "$work" "$name"
    result="$output/$name.tar.gz"
    ;;
  windows)
    windows_package "$output/$name.exe"
    result="$output/$name.exe"
    ;;
esac

run rm -rf "$work"
if [ "$DRY_RUN" != "1" ]; then
  [ -s "$result" ] || die "packaging did not produce $result"
fi
note "packaged $result"
