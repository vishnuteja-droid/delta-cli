#!/bin/sh
# CRITERION: C3 delta/bin/install succeeds from a scratch $HOME that has never seen delta, writing all 20 files it claims to
set -eu
cd "${DELTA_ROOT:-$PWD}"

tmp_home=$(mktemp -d); tmp_log=$(mktemp)
trap 'rm -rf "$tmp_home"; rm -f "$tmp_log"' EXIT

# A machine that has never seen delta: no pre-existing command directories
# at all under $HOME. Log outside $HOME so it isn't counted as install output.
set +e
HOME=$tmp_home delta/bin/install >"$tmp_log" 2>&1
rc=$?
set -e
[ "$rc" -eq 0 ] || { echo "install exited $rc on a fresh \$HOME"; cat "$tmp_log"; exit 1; }

expected="
.claude/commands/delta-explore.md
.claude/commands/delta-propose.md
.claude/commands/delta-apply.md
.claude/commands/delta-verify.md
.claude/commands/delta-archive.md
.gemini/commands/delta/explore.toml
.gemini/commands/delta/propose.toml
.gemini/commands/delta/apply.toml
.gemini/commands/delta/verify.toml
.gemini/commands/delta/archive.toml
.gemini/config/skills/delta-explore.md
.gemini/config/skills/delta-propose.md
.gemini/config/skills/delta-apply.md
.gemini/config/skills/delta-verify.md
.gemini/config/skills/delta-archive.md
.codex/prompts/delta-explore.md
.codex/prompts/delta-propose.md
.codex/prompts/delta-apply.md
.codex/prompts/delta-verify.md
.codex/prompts/delta-archive.md
"

missing=0
for f in $expected; do
    [ -f "$tmp_home/$f" ] || { echo "install did not write $f"; missing=1; }
done
[ "$missing" -eq 0 ] || exit 1

count=$(find "$tmp_home" -type f | wc -l | tr -d ' ')
[ "$count" -eq 20 ] || { echo "expected exactly 20 files written, found $count"; exit 1; }

exit 0
