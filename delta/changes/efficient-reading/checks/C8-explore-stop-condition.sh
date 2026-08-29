#!/bin/sh
# CRITERION: C8 explore has an explicit stop condition (entry point, call chain, data touched, unknowns, then stop) - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "Stop when you have enough" "$f" || { echo "$f has no explicit stop-condition heading"; fail=1; }
    grep -qi "established, stop" "$f" || { echo "$f does not say to stop once the four things are established"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
