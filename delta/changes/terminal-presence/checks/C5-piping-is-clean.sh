#!/bin/sh
# CRITERION: C5 piping to a file produces clean plain text with no escape sequences and no carriage returns
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 a thing"
add_check "$tmp/repo" fx C1 'exit 0'

( cd "$tmp/repo" && COLORTERM=truecolor delta/bin/verify fx ) > "$tmp/out.txt" 2>&1

grep -qP '\x1b' "$tmp/out.txt" 2>/dev/null && { echo "ESC byte leaked into piped output"; exit 1; }
grep -qP '\x0d' "$tmp/out.txt" 2>/dev/null && { echo "carriage return leaked into piped output"; exit 1; }
grep -q 'delta verify' "$tmp/out.txt" || { echo "the frame is missing from piped output"; exit 1; }

exit 0
