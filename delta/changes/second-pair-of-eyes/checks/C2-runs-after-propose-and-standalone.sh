#!/bin/sh
# CRITERION: C2 critique runs at the end of propose and can be re-run independently
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0

files="delta/commands/propose.md .claude/commands/delta-propose.md .codex/prompts/delta-propose.md .agents/skills/delta-propose.md .gemini/commands/delta/propose.toml"
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "run.*critique\|then critique" "$f" || { echo "$f does not invoke critique"; fail=1; }
done

# Standalone re-run: critique.md itself must resolve a change id the same
# way every other re-runnable command does (an argument, or the most
# recently modified change), not require propose to hand it anything.
cf=delta/commands/critique.md
grep -q 'argument-hint: "\[change-id\]"' "$cf" || { echo "$cf has no [change-id] argument-hint - can't tell it's independently invocable"; fail=1; }
grep -qi "most recently modified" "$cf" || { echo "$cf does not default to the most recently modified change when re-run standalone"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
