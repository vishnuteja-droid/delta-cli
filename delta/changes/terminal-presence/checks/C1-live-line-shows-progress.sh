#!/bin/sh
# CRITERION: C1 the status line updates in place and shows the current file or check, not just a spinner
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"
have_script || { echo "no script(1) on this machine - cannot drive a real pty"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 a slow thing"
add_check "$tmp/repo" fx C1 'sleep 1.2'

out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "delta/bin/verify fx" /dev/null </dev/null 2>/dev/null) || true

# The live line must name the criterion (not just spin) and show an
# elapsed-time field of the form M:SS - both required, not just a spinner.
printf '%s' "$out" | grep -q 'running' || { echo "no verb on the live line"; exit 1; }
printf '%s' "$out" | grep -q 'C1' || { echo "the live line never named the check"; exit 1; }
printf '%s' "$out" | grep -Eq '[0-9]:[0-9][0-9]' || { echo "no elapsed-time field on the live line"; exit 1; }
# In-place rewriting: a carriage return, not a newline, between live-line frames.
printf '%s' "$out" | grep -qP '\x0d' 2>/dev/null || printf '%s' "$out" | grep -q "$(printf '\r')" || {
    echo "no carriage-return rewriting seen - the line is not updating in place"; exit 1; }

exit 0
