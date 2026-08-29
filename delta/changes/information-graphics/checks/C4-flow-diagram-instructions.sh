#!/bin/sh
# CRITERION: C4 explore's instructions require inline unknowns, the 80-column width rule with a prose-chain fallback, and a Mermaid export
set -eu
cd "${DELTA_ROOT:-$PWD}"

f=delta/commands/explore.md
fail=0

grep -qi "80 column" "$f" || { echo "$f does not state the 80-column width rule"; fail=1; }
grep -qi "prose chain" "$f" || { echo "$f does not mention the prose-chain fallback"; fail=1; }
grep -q "flow.mmd" "$f" || { echo "$f does not say to write a Mermaid file"; fail=1; }
grep -q "flowchart" "$f" || { echo "$f has no Mermaid flowchart example"; fail=1; }

# "unknowns marked inline where they occur" is the CR's own phrasing for the
# thing that distinguishes this from a bulleted list at the bottom.
grep -qi "inline" "$f" || { echo "$f does not say unknowns go inline in the diagram"; fail=1; }

# The same instruction has to reach every generated CLI, or only one tool's
# agent would ever draw the diagram correctly.
for g in .claude/commands/delta-explore.md .codex/prompts/delta-explore.md \
         .agents/skills/delta-explore.md; do
    grep -q "flow.mmd" "$g" || { echo "$g does not carry the Mermaid-export instruction"; fail=1; }
done
grep -q "flow.mmd" .gemini/commands/delta/explore.toml \
    || { echo ".gemini/commands/delta/explore.toml does not carry the Mermaid-export instruction"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
