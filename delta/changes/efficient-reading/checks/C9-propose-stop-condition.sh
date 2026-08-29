#!/bin/sh
# CRITERION: C9 propose has an explicit stop condition tied to measurability, not a subjective "complete" - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/propose.md .claude/commands/delta-propose.md .codex/prompts/delta-propose.md .agents/skills/delta-propose.md .gemini/commands/delta/propose.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "Stop when you have enough" "$f" || { echo "$f has no explicit stop-condition heading"; fail=1; }
    grep -qi "checked-or-MANUAL\|has a check or is marked MANUAL" "$f" \
        || { echo "$f's stop condition is not tied to measurable+checked-or-MANUAL"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
