#!/bin/sh
# CRITERION: C3 the same check passing flips it to a normal check and writes a timestamped record to run/
set -eu
DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/reproduction-first-checks/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
bug_fixture "$tmp" "# EXPECT: fail-until-fixed"

run_verify "$tmp" >/dev/null                       # reproduce it first
touch "$tmp/FIXED"                                 # the fix lands
got=$(run_verify "$tmp")
test "$got" = 0 || { echo "a fixed reproduction should exit 0, got $got"; exit 1; }

# The flip is permanent: the EXPECT line no longer says fail-until-fixed.
grep -q '^# EXPECT: fail-until-fixed' "$tmp/checks/B1-repro.sh" && {
    echo "the check was not flipped; it still expects failure"; exit 1; }

# The record carries a timestamp and both events.
grep -qE '^B1 fixed: [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}-[0-9]{2}-[0-9]{2} - reproduced [0-9]{4}-[0-9]{2}-[0-9]{2}T' \
    "$tmp/run/reproductions.md" || {
    echo "no timestamped fix record linking back to the reproduction"; exit 1; }

# Having flipped, it is an ordinary regression guard: breaking it again fails.
rm "$tmp/FIXED"
got=$(run_verify "$tmp")
test "$got" = 1 || { echo "a flipped check should fail like any other, got $got"; exit 1; }
