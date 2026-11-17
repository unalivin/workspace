#!/usr/bin/env bash

REAL_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NOREPLY_DOMAIN="users.noreply.github.com"

make_workspace() {
  TEST_ROOT="$(mktemp -d)"
  export WORKSPACE_ROOT="$TEST_ROOT/ws"
  export REGISTRY_FILE="$WORKSPACE_ROOT/scripts/projects.tsv"
  export GIT_CONFIG_GLOBAL="$TEST_ROOT/gitconfig"
  export GIT_CONFIG_NOSYSTEM=1
  unset GITHUB_ACTIONS DRY_RUN
  mkdir -p "$WORKSPACE_ROOT/templates" "$WORKSPACE_ROOT/.github/workflows" "$WORKSPACE_ROOT/.github/ISSUE_TEMPLATE" "$WORKSPACE_ROOT/docs"
  cp -R "$REAL_ROOT/scripts" "$WORKSPACE_ROOT/scripts"
  cp -R "$REAL_ROOT/templates/workflows" "$WORKSPACE_ROOT/templates/workflows"
  cp -R "$REAL_ROOT/templates/pull-request" "$WORKSPACE_ROOT/templates/pull-request"
  awk -F'\t' -v OFS='\t' 'NR > 1 { $5 = "Not started" } { print }' "$REAL_ROOT/scripts/projects.tsv" >"$REGISTRY_FILE"
  printf '# workspace\n\n<!-- projects:start -->\n<!-- projects:end -->\n\n<!-- destinations:start -->\n<!-- destinations:end -->\n' >"$WORKSPACE_ROOT/README.md"
  local item
  for item in LICENSE CONTRIBUTING.md SECURITY.md CODE_OF_CONDUCT.md CHANGELOG.md .editorconfig .gitattributes Makefile docs/LANGUAGES.md docs/ADD_PROJECT.md docs/RELEASING.md; do
    printf 'placeholder\n' >"$WORKSPACE_ROOT/$item"
  done
  for item in docs/WORKSPACE.md docs/ROADMAP.md; do
    printf '# document\n\n<!-- destinations:start -->\n<!-- destinations:end -->\n' >"$WORKSPACE_ROOT/$item"
  done
  printf 'dist/\n' >"$WORKSPACE_ROOT/.gitignore"
  git config --global user.name "Workspace Tester"
  git config --global user.email "tester@${NOREPLY_DOMAIN}"
  git config --global init.defaultBranch main
  git config --global advice.detachedHead false
  git -C "$WORKSPACE_ROOT" init --quiet
  "$WORKSPACE_ROOT/scripts/sync-readme.sh" >/dev/null
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh" >/dev/null
  commit_all "chore: initial public release"
}

commit_all() {
  git -C "$WORKSPACE_ROOT" add -A
  git -C "$WORKSPACE_ROOT" commit --quiet -m "$1"
}

remove_workspace() {
  if [ -n "${TEST_ROOT:-}" ] && [ -d "$TEST_ROOT" ]; then
    rm -rf "$TEST_ROOT"
  fi
}

make_fake_template() {
  local language="$1" base="$WORKSPACE_ROOT/templates/$1" item
  make_fake_workflow_templates "$language"
  mkdir -p "$base/cmd/templateapp" "$base/migrations" "$base/scripts" "$base/assets" "$base/testdata" "$base/docs/adr" "$base/.github" "$base/internal/meta"
  for item in CHANGELOG.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md .gitignore .gitattributes .env.example docs/DESIGN.md docs/README.md docs/CONTRIBUTING.md docs/INSTALLATION.md docs/USAGE.md docs/COMMANDS.md docs/CONFIGURATION.md docs/TROUBLESHOOTING.md docs/SECURITY.md docs/THREAT_MODEL.md docs/PLATFORM_SUPPORT.md docs/TESTING.md docs/ARCHITECTURE.md docs/ROADMAP.md docs/RELEASING.md docs/RELEASE_NOTES.md docs/LICENSES.md scripts/publish-github.sh scripts/dev-setup.sh scripts/check.sh scripts/demo.sh scripts/package.sh assets/icon.svg testdata/sample.txt docs/adr/0001.md; do
    printf 'placeholder\n' >"$base/$item"
  done
  printf 'CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;\n' >"$base/migrations/0001_init.sql"
  printf 'MIT License\n\nCopyright (c) 2026 Unalivin\n\nPermission is hereby granted.\n' >"$base/LICENSE"
  printf '# Third-party notices\n\nTemplate App is released under the MIT license, Copyright (c) 2026 Unalivin.\n' >"$base/THIRD_PARTY_NOTICES.md"
  printf 'APP_SLUG="templateapp"\nAPP_PUBLISHER="Unalivin"\n' >"$base/scripts/lib.sh"
  printf 'package meta\n\nconst (\n\tModule    = "github.com/unalivin/templateapp"\n\tAppID     = "io.github.unalivin.templateapp"\n\tPublisher = "Unalivin"\n)\n' >"$base/internal/meta/meta.go"
  printf "## Checklist\n\n- [ ] The title uses the scope \`templateapp\`\n- [ ] \`make check\` passes for Template App\n" >"$base/.github/PULL_REQUEST_TEMPLATE.md"
  {
    printf '# Template App\n\nTEMPLATE_DESCRIPTION\n\n'
    printf 'Repository: https://github.com/unalivin/workspace\n'
    printf 'Folder: templates/%s\n' "$language"
    printf 'Release tag: templateapp/v0.1.0-dev\n\n'
    for item in Download Install Usage Documentation Security Contributing; do
      printf '## %s\n\ntext\n\n' "$item"
    done
    printf '## License\n\nMIT, Copyright (c) 2026 Unalivin.\n'
  } >"$base/README.md"
  {
    printf 'app=Template App\nslug=templateapp\nprefix=TEMPLATEAPP_UI\n'
    printf 'tagline=TEMPLATE_TAGLINE\ntopics=TEMPLATE_TOPICS\nicon=TEMPLATE_ICON_CONCEPT\n'
  } >"$base/cmd/templateapp/metadata.txt"
  {
    printf 'fmt:\nfmt-check:\nlint:\ntest:\nrace-or-sanitize:\nbuild:\nbuild-nogui:\nclean:\n'
    printf 'check:\n\t@test ! -f fail-marker\n\t@echo checked templateapp\n'
  } >"$base/Makefile"
}

make_fake_gh() {
  mkdir -p "$TEST_ROOT/bin"
  export GH_LOG="$TEST_ROOT/gh.log"
  export FAKE_GH_LOGIN="${FAKE_GH_LOGIN:-unalivin}"
  export FAKE_GH_REPO_EXISTS="${FAKE_GH_REPO_EXISTS:-0}"
  : >"$GH_LOG"
  cat >"$TEST_ROOT/bin/gh" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$GH_LOG"
case "$1 ${2:-}" in
  "api user")
    printf '%s\n' "$FAKE_GH_LOGIN"
    ;;
  "repo view")
    [ "$FAKE_GH_REPO_EXISTS" = "1" ]
    ;;
esac
FAKE
  chmod +x "$TEST_ROOT/bin/gh"
  export PATH="$TEST_ROOT/bin:$PATH"
}

make_bare_remote() {
  REMOTE_DIR="$TEST_ROOT/remote.git"
  git init --quiet --bare "$REMOTE_DIR"
}

make_fake_workflow_templates() {
  local language="$1" kind
  for kind in ci release; do
    if [ ! -f "$WORKSPACE_ROOT/templates/workflows/$language-$kind.yml" ]; then
      {
        printf 'name: %s __NAME__\n' "$kind"
        printf 'app: __APP_NAME__\nworkdir: __WORKDIR__\npaths: __PATH_FILTER__\n'
        printf 'tags: __TAG_PATTERN__\nself: __SELF__\n'
      } >"$WORKSPACE_ROOT/templates/workflows/$language-$kind.yml"
    fi
  done
}

refute() {
  if "$@"; then
    return 1
  fi
}
