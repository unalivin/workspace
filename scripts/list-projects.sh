#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/list-projects.sh [--table|--markdown|--tsv|--folders|--existing] [--language go|rust|cpp]

Prints the workspace project registry.

  --table      Aligned plain-text table (default)
  --markdown   Markdown table used in the root README
  --tsv        Tab-separated rows: number, folder, app, language, status
  --folders    Folder names only, one per line
  --existing   Folder names whose directory exists, one per line
  --language   Restrict output to one primary language
  --help       Show this help
USAGE
}

format="table"
language=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --table) format="table" ;;
    --markdown) format="markdown" ;;
    --tsv) format="tsv" ;;
    --folders) format="folders" ;;
    --existing) format="existing" ;;
    --language)
      [ "$#" -ge 2 ] || die "--language needs a value"
      language="$2"
      valid_language "$language" || die "unknown language: $language (use go, rust or cpp)"
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

rows() {
  if [ -n "$language" ]; then
    registry_rows | awk -F'\t' -v language="$language" '$4 == language'
  else
    registry_rows
  fi
}

case "$format" in
  folders)
    rows | cut -f2
    ;;
  existing)
    rows | cut -f2 | while IFS= read -r folder; do
      if [ -d "$WORKSPACE_ROOT/$folder" ]; then
        printf '%s\n' "$folder"
      fi
    done
    ;;
  tsv)
    rows | cut -f1-5
    ;;
  markdown)
    printf '| # | Folder | App | Language | Description | Status | Docs |\n'
    printf '|---|---|---|---|---|---|---|\n'
    rows | while IFS=$'\t' read -r number folder app lang status tagline _ _ _; do
      if [ -d "$WORKSPACE_ROOT/$folder" ]; then
        docs="[docs]($folder/docs/README.md)"
        link="[\`$folder\`]($folder/)"
      else
        docs="-"
        link="\`$folder\`"
      fi
      printf '| %s | %s | %s | %s | %s | %s | %s |\n' "$number" "$link" "$app" "$(language_label "$lang")" "$tagline" "$status" "$docs"
    done
    ;;
  table)
    printf '%-3s %-18s %-18s %-5s %-12s %s\n' "#" "FOLDER" "APP" "LANG" "STATUS" "DESCRIPTION"
    rows | while IFS=$'\t' read -r number folder app lang status tagline _ _ _; do
      printf '%-3s %-18s %-18s %-5s %-12s %s\n' "$number" "$folder" "$app" "$(language_label "$lang")" "$status" "$tagline"
    done
    ;;
esac
