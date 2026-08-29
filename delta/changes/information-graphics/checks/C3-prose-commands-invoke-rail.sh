#!/bin/sh
# CRITERION: C3 explore, propose, apply, and archive each instruct printing the same delta/bin/stage-rail output, not a description of their own
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0
for f in explore propose apply archive; do
    grep -q "delta/bin/stage-rail $f" "delta/commands/$f.md" \
        || { echo "delta/commands/$f.md does not invoke stage-rail $f"; fail=1; }
done

# Every generated per-CLI file has to carry the same instruction, or the
# rail would only ever appear for one CLI.
for f in .claude/commands/delta-explore.md .claude/commands/delta-propose.md \
         .claude/commands/delta-apply.md .claude/commands/delta-archive.md \
         .codex/prompts/delta-explore.md .codex/prompts/delta-propose.md \
         .codex/prompts/delta-apply.md .codex/prompts/delta-archive.md \
         .agents/skills/delta-explore.md .agents/skills/delta-propose.md \
         .agents/skills/delta-apply.md .agents/skills/delta-archive.md; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -q "delta/bin/stage-rail" "$f" || { echo "$f does not mention stage-rail"; fail=1; }
done
for f in .gemini/commands/delta/explore.toml .gemini/commands/delta/propose.toml \
         .gemini/commands/delta/apply.toml .gemini/commands/delta/archive.toml; do
    [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
    grep -q "delta/bin/stage-rail" "$f" || { echo "$f does not mention stage-rail"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
