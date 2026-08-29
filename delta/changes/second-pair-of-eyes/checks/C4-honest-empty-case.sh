#!/bin/sh
# CRITERION: C4 critique reports finding nothing rather than inventing objections - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/critique.md .claude/commands/delta-critique.md .codex/prompts/delta-critique.md .agents/skills/delta-critique.md .gemini/commands/delta/critique.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -q "No findings\." "$f" || { echo "$f has no literal 'No findings.' instruction"; fail=1; }
    grep -qi "do not force an entry\|manufacturing an objection" "$f" \
        || { echo "$f does not warn against manufacturing objections to look thorough"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
