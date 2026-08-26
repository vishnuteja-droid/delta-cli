#!/bin/sh
# CRITERION: C1 a check with no EXPECT header behaves exactly as it did before this change
set -eu
DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/reproduction-first-checks/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

# No EXPECT line at all: passing is 0, failing is 1, exactly as before.
bug_fixture "$tmp" ""
got=$(run_verify "$tmp")
test "$got" = 1 || { echo "a failing check with no EXPECT should exit 1, got $got"; exit 1; }

touch "$tmp/FIXED"
got=$(run_verify "$tmp")
test "$got" = 0 || { echo "a passing check with no EXPECT should exit 0, got $got"; exit 1; }

# It is never reported as reproduced, and no ledger is created for it.
run_verify_out "$tmp" | grep -qi 'reproduc' && {
    echo "a check with no EXPECT was reported as a reproduction"; exit 1; }
test ! -f "$tmp/run/reproductions.md" || {
    echo "a check with no EXPECT wrote a reproduction ledger"; exit 1; }

# The delta's own example change still exits 0 unchanged.
got=$(run_verify example-verify-exit-codes)
test "$got" = 0 || { echo "the pre-existing example change now exits $got"; exit 1; }
