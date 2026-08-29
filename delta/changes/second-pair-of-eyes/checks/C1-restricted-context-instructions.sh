#!/bin/sh
# CRITERION: C1 critique reads the spec and constitution only; it has no access to exploration findings or the code - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/critique.md .claude/commands/delta-critique.md .codex/prompts/delta-critique.md .agents/skills/delta-critique.md .gemini/commands/delta/critique.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -q "spec.md.*and.*delta/constitution.md\|Read exactly two files" "$f" \
        || { echo "$f does not restrict reading to exactly spec.md and constitution.md"; fail=1; }
    grep -q "explore.md" "$f" || { echo "$f does not explicitly exclude explore.md"; fail=1; }
    grep -qi "original intent" "$f" || { echo "$f does not explicitly exclude the original intent"; fail=1; }
    grep -qi "the code the spec describes" "$f" || { echo "$f does not explicitly exclude the code"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
