#!/bin/sh
# CRITERION: C10 a clean run and a failing run are distinguishable at a glance without reading the numbers
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"
have_script || { echo "no script(1) on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" clean "- C1 passes"
add_check "$tmp/repo" clean C1 'exit 0'
mk_change "$tmp/repo" dirty "- C1 fails"
add_check "$tmp/repo" dirty C1 'exit 1'

clean_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "delta/bin/verify clean" /dev/null </dev/null 2>/dev/null) || true
dirty_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "delta/bin/verify dirty" /dev/null </dev/null 2>/dev/null) || true

# Clean: the closing frame carries an accent-coloured sigil/rule - truecolor
# (38;2;...) was not forced here, so the accent falls back to its plain SGR
# 33 approximation (see palette.sh); either counts as "accented".
clean_closing=$(printf '%s' "$clean_out" | grep -aE '1 (criterion|criteria)' | tail -1)
printf '%s' "$clean_closing" | grep -qP '\x1b\[(38;2;|33m)' 2>/dev/null || {
    echo "clean run's closing frame carries no accent colour"; exit 1; }

# Failing: red appears in the closing summary line specifically (the failed
# count), and the closing frame itself is NOT accent-coloured.
dirty_closing=$(printf '%s' "$dirty_out" | grep -aE '1 (criterion|criteria)' | tail -1)
printf '%s' "$dirty_closing" | grep -qP '\x1b\[31m' 2>/dev/null || {
    echo "failing run's summary has no red-highlighted count"; exit 1; }
printf '%s' "$dirty_closing" | grep -qP '\x1b\[(38;2;|33m)' 2>/dev/null && {
    echo "failing run's closing frame was accent-coloured - should only be the count"; exit 1; }

exit 0
