#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
measure="$(scripts/measure.sh logforge-siem)"
printf '%s\n' "$measure"
comment_hits="$(grep -RInE '^[[:space:]]*(//|///|//!|/\\*|\\*\\/)' logforge-siem/crates || true; grep -nE '^[[:space:]]*(--|/\\*|\\*\\/)' logforge-siem/migrations/*.sql || true)"
comment_status="empty"
if [[ -n "$comment_hits" ]]; then
  comment_status="matches found"
fi
python3 - "$measure" "$comment_status" <<'PY'
import re
import sys
from pathlib import Path

measure = sys.argv[1]
comment_status = sys.argv[2]
audit_path = Path('.workspace/audit/logforge-siem.md')
status_path = Path('.workspace/STATUS.md')
audit = audit_path.read_text()
pattern = r'```text\nProject logforge-siem, language rust, size class L\n.*?\n```'
match = re.search(pattern, audit, flags=re.S)
if not match:
    raise SystemExit('audit measurement block not found')
audit = audit[:match.start()] + '```text\n' + measure + '\n```' + audit[match.end():]
audit_path.write_text(audit)
values = {}
for key, label in [('lines', 'production lines'), ('tests', 'test functions'), ('screens', 'screens in GUI and TUI'), ('migrations', 'migrations'), ('tables', 'SQL tables')]:
    match = re.search(rf'^{re.escape(label)}\s+(\d+)', measure, flags=re.M)
    if not match:
        raise SystemExit(f'measure output missing {label}')
    values[key] = int(match.group(1))
status = status_path.read_text()
new_summary = (
    f"Measure PASS: {values['lines']} lines, {values['tests']} tests, "
    f"{values['screens']} screens, {values['migrations']} migrations, "
    f"{values['tables']} SQL tables; comment scan {comment_status}; CI pending on current source; "
    "step 1 evidence pending, step 3 in progress |"
)
status, count = re.subn(r'Measure PASS: [^\n]+', new_summary, status, count=1)
if count != 1:
    raise SystemExit('LogForge-SIEM status summary not found')
status_path.write_text(status)
PY
