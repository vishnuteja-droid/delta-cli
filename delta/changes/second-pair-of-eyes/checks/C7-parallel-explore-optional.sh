#!/bin/sh
# CRITERION: C7 explore's parallelism is optional and produces identical findings either way - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "you may fan" "$f" || { echo "$f does not describe the optional parallel fan-out"; fail=1; }
    grep -qi "Optional, always" "$f" || { echo "$f does not say the fan-out is optional, always"; fail=1; }
    grep -qi "sequential is the reference implementation" "$f" \
        || { echo "$f does not name sequential as the reference implementation"; fail=1; }
    grep -qi "same findings a" "$f" \
        || { echo "$f does not say parallel output must match sequential output"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
