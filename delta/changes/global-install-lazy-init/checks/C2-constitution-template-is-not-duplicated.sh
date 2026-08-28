#!/bin/sh
# CRITERION: C2 the constitution template embedded in propose is identical to delta/constitution.md, and still reads as a template
set -eu

extract_between() {  # extract_between <file> <start-marker> <end-marker>
    awk -v s="$2" -v e="$3" '
        index($0, s) { grabbing=1; next }
        index($0, e) && grabbing { exit }
        grabbing { print }
    ' "$1"
}

# The canonical source carries a placeholder marker, not the content itself -
# that is what makes drift between delta/constitution.md and the copy every
# CLI ships impossible: there is only one copy, spliced in at generation time.
grep -q '{{CONSTITUTION_TEMPLATE}}' delta/commands/propose.md || {
    echo "delta/commands/propose.md no longer uses the {{CONSTITUTION_TEMPLATE}} marker - the template must be injected, never hand-copied"
    exit 1
}

want=$(cat delta/constitution.md)

for f in .claude/commands/delta-propose.md .agents/skills/delta-propose.md .codex/prompts/delta-propose.md .gemini/commands/delta/propose.toml; do
    got=$(extract_between "$f" '```markdown' '```')
    test "$got" = "$want" || { echo "$f: embedded constitution template does not match delta/constitution.md"; exit 1; }
done

grep -q 'Litmus test for every line' delta/constitution.md || {
    echo "delta/constitution.md no longer reads like a template with prompts"; exit 1; }
grep -q 'Replace everything below with your own rules' delta/constitution.md || {
    echo "delta/constitution.md dropped the instruction to replace it"; exit 1; }
