#!/bin/sh
# CRITERION: C1 no document under README.md, AGENTS.md, or delta/commands/ references a removed or never-shipped feature (ui.html, /delta:bug, an init command, CR-003)
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0

# ui.html was scoped out of CR-004 entirely and is not part of this repo -
# it should never be named as if it exists.
if grep -rn "ui\.html" README.md AGENTS.md delta/commands/*.md 2>/dev/null; then
    echo "ui.html referenced - it was never built"
    fail=1
fi

# CR-003 never existed as a shipped artifact in this repo's history.
if grep -rln "CR-003" README.md AGENTS.md delta/commands/*.md CHANGELOG.md 2>/dev/null; then
    echo "CR-003 referenced - no such change exists in this repo"
    fail=1
fi

# There is deliberately no bug command and no init command. The real
# invariant is that no such command file or generated adapter output
# exists - not a text search, which would false-positive on the correct
# negations already in the docs ("no /delta:bug command", "No init command").
if ls delta/commands/ | grep -viE '^(explore|propose|apply|verify|archive)\.md$'; then
    echo "delta/commands/ has a file outside the five known commands"
    fail=1
fi
for adapter_dir in .claude/commands .agents/skills .codex/prompts; do
    [ -d "$adapter_dir" ] || continue
    if ls "$adapter_dir" 2>/dev/null | grep -iE 'bug|^delta-init'; then
        echo "$adapter_dir has a generated bug/init command file"
        fail=1
    fi
done
if [ -d .gemini/commands/delta ] && ls .gemini/commands/delta | grep -iE 'bug|init'; then
    echo ".gemini/commands/delta has a generated bug/init command file"
    fail=1
fi

[ "$fail" -eq 0 ] && exit 0
exit 1
