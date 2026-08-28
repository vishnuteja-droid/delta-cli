#!/bin/sh
# CRITERION: C2 results print as they complete, above the status line
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"
have_script || { echo "no script(1) on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 fast one
- C2 slow one"
add_check "$tmp/repo" fx C1 'exit 0'
add_check "$tmp/repo" fx C2 'sleep 1; exit 0'

out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "delta/bin/verify fx" /dev/null </dev/null 2>/dev/null) || true

# C1's completed result (a real newline-terminated line, not the live \r
# line) must appear BEFORE C2 finishes - i.e. while C2 is still spinning.
c1_done_pos=$(printf '%s' "$out" | grep -abo 'C1 fast one' | tail -1 | cut -d: -f1)
c2_spin_pos=$(printf '%s' "$out" | grep -abo 'running.*C2' | head -1 | cut -d: -f1)
test -n "$c1_done_pos" || { echo "C1's completed result never appeared"; exit 1; }
test -n "$c2_spin_pos" || { echo "C2 never showed a live status line"; exit 1; }
test "$c1_done_pos" -lt "$c2_spin_pos" || {
    echo "C1's result did not appear above C2's still-running status line"; exit 1; }

exit 0
