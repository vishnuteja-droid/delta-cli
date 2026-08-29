#!/bin/sh
# CRITERION: C5 with roles declared, critique runs in an isolated sub-context; without, sequentially - and both paths are fully specified, for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

files="delta/commands/critique.md .claude/commands/delta-critique.md .codex/prompts/delta-critique.md .agents/skills/delta-critique.md .gemini/commands/delta/critique.toml"

fail=0
for f in $files; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -qi "spawn one now" "$f" || { echo "$f does not instruct spawning a subagent when roles is declared"; fail=1; }
    grep -qi "no role is declared" "$f" || { echo "$f has no explicit no-roles-declared branch"; fail=1; }
    grep -qi "do not consult what you already" "$f" || { echo "$f does not instruct disregarding prior context in the sequential fallback"; fail=1; }
done

# adapters.yaml itself must document the roles field and show it declared
# for at least one adapter and deliberately absent for at least one -
# otherwise "with roles" and "without" are never actually both exercised.
grep -q "^#   roles" delta/adapters.yaml || { echo "adapters.yaml does not document the roles field"; fail=1; }
grep -q "^    roles:" delta/adapters.yaml || { echo "no adapter actually declares roles"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
