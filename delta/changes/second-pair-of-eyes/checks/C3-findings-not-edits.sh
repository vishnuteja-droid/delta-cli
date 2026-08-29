#!/bin/sh
# CRITERION: C3 output is findings, never edits - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/critique.md .claude/commands/delta-critique.md .codex/prompts/delta-critique.md .agents/skills/delta-critique.md .gemini/commands/delta/critique.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "Never change" "$f" || { echo "$f does not say to never change spec.md or checks/"; fail=1; }
    grep -q "critique.md" "$f" || { echo "$f does not name critique.md as the findings output file"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
