#!/bin/sh
# CRITERION: C9 propose shows the spec and checks as a coloured diff
set -eu

grep -q '```diff' delta/commands/propose.md || {
    echo "propose.md no longer instructs a fenced diff block"; exit 1; }
grep -qi 'green add\|red remove\|dim context' delta/commands/propose.md || {
    echo "propose.md dropped the colour semantics (green add / red remove / dim context)"; exit 1; }

for f in .claude/commands/delta-propose.md .agents/skills/delta-propose.md \
         .codex/prompts/delta-propose.md .gemini/commands/delta/propose.toml; do
    grep -q '```diff' "$f" || { echo "$f: generated file lost the diff-fence instruction"; exit 1; }
done
