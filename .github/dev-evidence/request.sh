#!/usr/bin/env bash
set -uo pipefail

cd templates/cpp || exit 1
step() {
  local name="$1" status
  shift
  "$@" >"$EVIDENCE_DIR/$name.log" 2>&1
  status="$?"
  printf '%s exit status: %s\n' "$name" "$status"
}
step setup scripts/dev-setup.sh --install
python_command="python3"
command -v python3 >/dev/null 2>&1 || python_command="python"
user_scripts="$("$python_command" -c 'import os, sysconfig; print(sysconfig.get_path("scripts", "%s_user" % ("nt" if os.name == "nt" else "posix")))')"
if command -v cygpath >/dev/null 2>&1; then
  user_scripts="$(cygpath -u "$user_scripts")"
fi
export PATH="$user_scripts:$HOME/.local/bin:$HOME/.local/npm/node_modules/.bin:$PATH"
uname -a
cmake --version
step doctor scripts/dev-setup.sh
step fmt make fmt-check
step lint make lint
step lint-gui make lint-gui
step test make test
step sanitize make race-or-sanitize
step build make build
step nogui make build-nogui test-nogui
step generated make docs-check assets-check notices-check
case "$(uname -s)" in
  Linux*)
    step vuln make vuln
    step fuzz make fuzz
    step shell make shell
    ;;
esac
step screenshots make screenshots
step package make package
step demo scripts/demo.sh --ui none
ls -la dist bin 2>&1
mkdir -p "$EVIDENCE_DIR/screens" "$EVIDENCE_DIR/vcpkg"
cp dist/screens/*.png "$EVIDENCE_DIR/screens/" 2>/dev/null || true
for file in build/*/vcpkg-manifest-install.log; do
  [ -f "$file" ] || continue
  cp "$file" "$EVIDENCE_DIR/vcpkg/$(basename "$(dirname "$file")")-install.log"
done
find .tools/vcpkg/buildtrees -maxdepth 2 -name '*.log' -size -400k 2>/dev/null | while IFS= read -r file; do
  cp "$file" "$EVIDENCE_DIR/vcpkg/$(basename "$(dirname "$file")")-$(basename "$file")"
done
exit 0
