#!/bin/sh
# CRITERION: C2 verify exits 1 when a check fails
set -eu

DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/example-verify-exit-codes/checks}/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_change "$tmp" "- C1 passes
- C2 fails"
add_check "$tmp" C1 0
add_check "$tmp" C2 1

got=$(run_verify "$tmp")
test "$got" = 1 || { echo "expected exit 1, got $got"; exit 1; }

# A failure must outrank a missing check and an open manual criterion.
tmp2=$(mktemp -d)
trap 'rm -rf "$tmp" "$tmp2"' EXIT
make_change "$tmp2" "- C1 fails
- C2 has no check
- C3 MANUAL unsigned"
add_check "$tmp2" C1 1

got=$(run_verify "$tmp2")
test "$got" = 1 || { echo "expected failure to outrank codes 2 and 3, got $got"; exit 1; }
