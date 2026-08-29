#!/bin/sh
# CRITERION: C3 delta/bin/install succeeds from a scratch $HOME that has never seen delta, writing one file per command per adapter
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

# Derived from delta/commands/*.md, not hardcoded - a new command (like
# critique in CR-008) must not silently break this check the way a fixed
# list and a fixed count both did once already.
cmds=$(for f in delta/commands/*.md; do b=${f##*/}; printf '%s\n' "${b%.md}"; done)
cmd_count=$(printf '%s\n' "$cmds" | grep -c .)

missing=0
for cmd in $cmds; do
    for f in ".claude/commands/delta-$cmd.md" \
             ".gemini/commands/delta/$cmd.toml" \
             ".gemini/config/skills/delta-$cmd.md" \
             ".codex/prompts/delta-$cmd.md"; do
        [ -f "$tmp_home/$f" ] || { echo "install did not write $f"; missing=1; }
    done
done
[ "$missing" -eq 0 ] || exit 1

count=$(find "$tmp_home" -type f | wc -l | tr -d ' ')
expected=$((cmd_count * 4))
[ "$count" -eq "$expected" ] || { echo "expected exactly $expected files written (${cmd_count} commands x 4 adapters), found $count"; exit 1; }

exit 0
