#!/bin/sh
# CRITERION: C5 explore records a service call as an edge (not followed) and skips tests/generated/vendored code unless named - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "recorded as an edge" "$f" || { echo "$f does not say a service call is recorded as an edge"; fail=1; }
    grep -qi "not followed" "$f" || { echo "$f does not say the call is not followed"; fail=1; }
    grep -qi "skip tests, generated code, and vendored" "$f" || { echo "$f does not say to skip tests/generated/vendored code"; fail=1; }
    grep -qi "intent names them" "$f" || { echo "$f does not scope the skip to what the intent names"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
