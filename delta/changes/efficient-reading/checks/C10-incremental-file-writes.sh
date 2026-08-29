#!/bin/sh
# CRITERION: C10 explore says to append findings to the file as each is established, not compose once at the end, and says why - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "Write incrementally" "$f" || { echo "$f has no 'Write incrementally' instruction"; fail=1; }
    grep -qi "gets interrupted" "$f" || { echo "$f does not say why (an interrupted run should leave something useful)"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
