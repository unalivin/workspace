#!/usr/bin/env bash
set -uo pipefail

cd templates/cpp || exit 1
scripts/dev-setup.sh --install >"$EVIDENCE_DIR/setup.log" 2>&1
printf 'setup exit status: %s\n' "$?"
python_command="python3"
command -v python3 >/dev/null 2>&1 || python_command="python"
user_scripts="$("$python_command" -c 'import os, sysconfig; print(sysconfig.get_path("scripts", "%s_user" % ("nt" if os.name == "nt" else "posix")))')"
if command -v cygpath >/dev/null 2>&1; then
  user_scripts="$(cygpath -u "$user_scripts")"
fi
export PATH="$user_scripts:$HOME/.local/bin:$PATH"
make build >"$EVIDENCE_DIR/build.log" 2>&1
printf 'build exit status: %s\n' "$?"
ls -la dist/bin

scratch="$(mktemp -d)"
export TEMPLATEAPP_DB="$scratch/window.db"
export TEMPLATEAPP_CONFIG_DIR="$scratch/config"

capture() {
  case "$(uname -s)" in
    Linux*) import -window root "$EVIDENCE_DIR/$1.png" || xwd -root -silent -out "$EVIDENCE_DIR/$1.xwd" ;;
    Darwin*) screencapture -x "$EVIDENCE_DIR/$1.png" ;;
    *) printf 'no capture on this system\n' ;;
  esac
}

window() {
  local name="$1" pid status
  shift
  "$@" >"$EVIDENCE_DIR/$name.log" 2>&1 &
  pid="$!"
  sleep 10
  if kill -0 "$pid" 2>/dev/null; then
    printf '%s: the process is alive after 10 seconds\n' "$name"
    capture "$name"
    kill "$pid"
    wait "$pid"
    printf '%s: exit status after the stop signal: %s\n' "$name" "$?"
  else
    wait "$pid"
    status="$?"
    printf '%s: exited by itself with status %s\n' "$name" "$status"
  fi
  cat "$EVIDENCE_DIR/$name.log"
}

program="dist/bin/templateapp"
case "$(uname -s)" in
  Linux*)
    sudo apt-get install -y --no-install-recommends xvfb imagemagick x11-apps libgl1-mesa-dri >"$EVIDENCE_DIR/apt.log" 2>&1
    printf 'apt exit status: %s\n' "$?"
    window no-display env -u DISPLAY -u WAYLAND_DISPLAY "$program" --ui gui
    Xvfb :99 -screen 0 1400x900x24 >"$EVIDENCE_DIR/xvfb.log" 2>&1 &
    server="$!"
    sleep 3
    export DISPLAY=:99
    window dashboard "$program" --ui gui
    "$program" demo load
    window dashboard-demo "$program" --ui gui
    window chooser "$program"
    kill "$server"
    ;;
  Darwin*)
    window dashboard "$program" --ui gui
    "$program" demo load
    window dashboard-demo "$program" --ui gui
    ;;
  *)
    window dashboard "$program.exe" --ui gui
    ;;
esac
"$program" --help | head -n 30
exit 0
