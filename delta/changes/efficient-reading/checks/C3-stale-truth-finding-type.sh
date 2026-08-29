#!/bin/sh
# CRITERION: C3 explore makes a stale truth entry its own named finding type, distinct from an unknown - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -q "^     stale" "$f" || { echo "$f has no 'stale' example line in its terminal-output section"; fail=1; }
    grep -qi "code wins" "$f" || { echo "$f does not say code wins over stale truth"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
