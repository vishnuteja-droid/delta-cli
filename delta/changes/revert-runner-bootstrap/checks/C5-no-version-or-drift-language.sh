#!/bin/sh
# CRITERION: C5 no message anywhere refers to runner versions, drift, or updating
#
# Scoped to the functional surfaces only: the shipped scripts and every
# generated command file, where any hit is unambiguous - this text drives
# behaviour or is read as an instruction, so the phrase can only mean the
# feature still exists. README.md and AGENTS.md are prose that legitimately
# explains what was removed and why, using the same vocabulary to say so -
# grep cannot tell "this exists" from "here's why it doesn't" in free text,
# so those are covered by the MANUAL criterion's read instead.
set -eu

targets="delta/bin/verify delta/bin/install delta/bin/generate-commands
delta/commands/explore.md delta/commands/propose.md delta/commands/apply.md
delta/commands/archive.md delta/commands/verify.md
.claude/commands/delta-explore.md .claude/commands/delta-propose.md
.claude/commands/delta-apply.md .claude/commands/delta-archive.md
.claude/commands/delta-verify.md"

hit=0
for f in $targets; do
    [ -f "$f" ] || continue
    pattern='runner_version|RUNNER_VERSION|version mismatch|global runner|\.delta/bin/verify|bootstrapped .*verify|runner drift|version drift'
    grep -Eiq "$pattern" "$f" && {
        echo "found stale version/drift/bootstrap language in $f:"
        grep -Ein "$pattern" "$f"
        hit=1
    }
done
test "$hit" -eq 0 || exit 1
