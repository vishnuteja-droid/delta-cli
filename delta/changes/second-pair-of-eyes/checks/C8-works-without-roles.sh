#!/bin/sh
# CRITERION: C8 delta works fully in a CLI with no roles support
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0

# Gemini CLI is the real, shipped example of an adapter with no roles - the
# CR states this as fact, and this table has to actually reflect it, not
# just every OTHER adapter opting out for unrelated reasons. Extract the
# whole gemini block (up to the next "- id:" line, not just one line after
# it) so a roles: entry anywhere in the block is actually seen.
gemini_block=$(awk '
    /^  - id: gemini/ { grab = 1 }
    grab && /^  - id:/ && !/gemini/ { exit }
    grab { print }
' delta/adapters.yaml)
if printf '%s\n' "$gemini_block" | grep -q "^    roles:"; then
    echo "gemini adapter unexpectedly declares roles - it should be the no-roles example"
    fail=1
fi

# critique's own instructions cannot make the no-roles path read as a
# degraded stub - it has to be fully specified: a concrete output file, a
# concrete rule for what to do with prior context, no step that only makes
# sense if a subagent mechanism exists.
grep -q "no role is declared" delta/commands/critique.md \
    || { echo "critique.md has no fully-specified sequential path"; fail=1; }
grep -q "delta/changes/<id>/critique.md" delta/commands/critique.md \
    || { echo "critique.md's output path is not concretely named, independent of roles"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
