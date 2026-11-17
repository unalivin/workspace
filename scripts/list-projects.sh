#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/list-projects.sh [--table|--markdown|--destinations|--tsv|--folders|--existing]
                                [--language go|rust|cpp] [--destination sonix-devs|personal]

Prints the workspace project registry.

  --table         Aligned plain-text table (default)
  --markdown      Markdown table used in the root README
  --destinations  Markdown table of project, language and destination, the
                  company projects first, used in the root README,
                  docs/WORKSPACE.md and docs/ROADMAP.md
  --tsv           Tab-separated rows: number, folder, app, language, status,
                  destination, copyright holder
  --folders       Folder names only, one per line
  --existing      Folder names whose directory exists, one per line
  --language      Restrict output to one primary language
  --destination   Restrict output to one destination
  --help          Show this help
USAGE
}

format="table"
language=""
destination=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --table) format="table" ;;
    --markdown) format="markdown" ;;
    --tsv) format="tsv" ;;
    --folders) format="folders" ;;
    --existing) format="existing" ;;
    --destinations) format="destinations" ;;
    --destination)
      [ "$#" -ge 2 ] || die "--destination needs a value"
      destination="$2"
      valid_destination "$destination" || die "unknown destination: $destination (use sonix-devs or personal)"
      shift
      ;;
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
  registry_rows | awk -F'\t' -v language="$language" -v destination="$destination" \
    '(language == "" || $4 == language) && (destination == "" || $10 == destination)'
}

destination_rows() {
  local wanted="$1" folder lang
  rows | awk -F'\t' -v wanted="$wanted" '$10 == wanted { print $2 "\t" $4 }' | while IFS=$'\t' read -r folder lang; do
    printf "| \`%s\` | %s | %s |\n" "$folder" "$(language_label "$lang")" "$(destination_label "$wanted")"
  done
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
    rows | cut -f1-5,10,11
    ;;
  destinations)
    company="$(rows | awk -F'\t' '$10 == "sonix-devs"' | wc -l | tr -d ' ')"
    personal="$(rows | awk -F'\t' '$10 == "personal"' | wc -l | tr -d ' ')"
    printf '%s projects will be published in the GitHub organization [%s](https://github.com/%s), which belongs to the company. %s projects stay on the personal profile [%s](https://github.com/%s). The copyright holder follows the destination: %s for the company projects, %s for the personal ones.\n\n' \
      "$company" "$COMPANY_OWNER" "$COMPANY_OWNER" "$personal" "$WORKSPACE_OWNER" "$WORKSPACE_OWNER" "$COMPANY_HOLDER" "$PERSONAL_HOLDER"
    printf '| Project | Language | Destination |\n'
    printf '|---|---|---|\n'
    destination_rows sonix-devs
    destination_rows personal
    ;;
  markdown)
    printf '| # | Folder | App | Language | Description | Status | Docs |\n'
    printf '|---|---|---|---|---|---|---|\n'
    rows | while IFS=$'\t' read -r number folder app lang status tagline _; do
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
    printf '%-3s %-18s %-18s %-5s %-12s %-11s %s\n' "#" "FOLDER" "APP" "LANG" "STATUS" "DESTINATION" "DESCRIPTION"
    rows | while IFS=$'\t' read -r number folder app lang status tagline _ _ _ target _; do
      printf '%-3s %-18s %-18s %-5s %-12s %-11s %s\n' "$number" "$folder" "$app" "$(language_label "$lang")" "$status" "$target" "$tagline"
    done
    ;;
esac
