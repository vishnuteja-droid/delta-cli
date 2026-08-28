#!/bin/sh
# CRITERION: C8 the gradient appears on a truecolor terminal and degrades silently elsewhere
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"
have_script || { echo "no script(1) on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 a thing"
add_check "$tmp/repo" fx C1 'exit 0'

with_tc=$(cd "$tmp/repo" && COLORTERM=truecolor LANG=en_US.UTF-8 script -qec "delta/bin/verify fx" /dev/null </dev/null 2>/dev/null) || true
printf '%s' "$with_tc" | grep -qP '38;2;\d+;\d+;\d+' 2>/dev/null || {
    echo "no 24-bit gradient escape found with COLORTERM=truecolor"; exit 1; }

without_tc=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "delta/bin/verify fx" /dev/null </dev/null 2>/dev/null) || true
printf '%s' "$without_tc" | grep -qP '38;2;\d+;\d+;\d+' 2>/dev/null && {
    echo "a 24-bit escape leaked with no COLORTERM set - should degrade to flat colour"; exit 1; }
# Degrading silently means no error text, and the frame still renders.
printf '%s' "$without_tc" | grep -qi 'error\|cannot\|no such' && {
    echo "degrading without truecolor produced visible error text"; exit 1; }
printf '%s' "$without_tc" | grep -q 'delta verify' || { echo "the frame vanished without truecolor"; exit 1; }

exit 0
