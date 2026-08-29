#!/bin/sh
# CRITERION: C7 explore states the cheap-first ladder in order and says to note when a body was read - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/explore.md .claude/commands/delta-explore.md .codex/prompts/delta-explore.md .agents/skills/delta-explore.md .gemini/commands/delta/explore.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    # The four steps must appear as a numbered list, in this exact order.
    awk '
        /grep for the symbol/ { g = NR }
        /file headers and imports/ { h = NR }
        /^[0-9]\. signatures$/ { s = NR }
        /^[0-9]\. bodies$/ { b = NR }
        END {
            if (!g || !h || !s || !b) { print "missing"; exit 1 }
            if (!(g < h && h < s && s < b)) { print "out of order"; exit 1 }
        }
    ' "$f" || { echo "$f does not state the ladder (grep, headers/imports, signatures, bodies) in order"; fail=1; }
    grep -qi "when a body had to be read" "$f" || { echo "$f does not say to note when a body had to be read"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
