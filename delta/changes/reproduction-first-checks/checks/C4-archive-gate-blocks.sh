#!/bin/sh
# CRITERION: C4 verify --archive-gate exits non-zero while a reproduction is outstanding, naming it
set -eu
DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/reproduction-first-checks/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
bug_fixture "$tmp" "# EXPECT: fail-until-fixed"

got=$(run_verify --archive-gate "$tmp")
test "$got" = 5 || { echo "archive gate should exit 5 while outstanding, got $got"; exit 1; }

out=$(run_verify_out --archive-gate "$tmp")
printf '%s' "$out" | grep -q 'cannot archive' || {
    echo "the gate did not say it was blocking the archive"; exit 1; }
printf '%s' "$out" | grep -q 'B1' || {
    echo "the gate did not name the outstanding criterion"; exit 1; }

# Without the flag the same state is fine, so the gate is opt-in.
got=$(run_verify "$tmp")
test "$got" = 0 || { echo "the gate should be opt-in; plain run gave $got"; exit 1; }

# Once fixed, the gate opens.
touch "$tmp/FIXED"
got=$(run_verify --archive-gate "$tmp")
test "$got" = 0 || { echo "archive gate should pass once fixed, got $got"; exit 1; }
