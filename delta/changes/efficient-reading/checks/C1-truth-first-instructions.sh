#!/bin/sh
# CRITERION: C1 explore reads truth before source, checks git log since truth's last commit, and investigates only the gap - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "before touching" "$f" || { echo "$f does not say to read truth before source"; fail=1; }
    grep -q "git log" "$f" || { echo "$f does not mention checking git log"; fail=1; }
    grep -qi "investigate only what truth" "$f" || { echo "$f does not scope investigation to the truth gap"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
