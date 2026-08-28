#!/bin/sh
# CRITERION: C6 NO_COLOR=1 on a TTY disables colour but keeps the frame and the results
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"
have_script || { echo "no script(1) on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 a thing"
add_check "$tmp/repo" fx C1 'exit 0'

out=$(cd "$tmp/repo" && NO_COLOR=1 LANG=en_US.UTF-8 script -qec "delta/bin/verify fx" /dev/null </dev/null 2>/dev/null) || true

printf '%s' "$out" | grep -qP '\x1b\[' 2>/dev/null && { echo "an escape sequence leaked despite NO_COLOR"; exit 1; }
printf '%s' "$out" | grep -q 'delta verify' || { echo "the frame disappeared under NO_COLOR"; exit 1; }
printf '%s' "$out" | grep -qi 'C1' || { echo "the result disappeared under NO_COLOR"; exit 1; }

exit 0
