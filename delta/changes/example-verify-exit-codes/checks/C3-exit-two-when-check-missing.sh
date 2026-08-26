#!/bin/sh
# CRITERION: C3 verify exits 2 when a criterion has no corresponding check
set -eu

DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/example-verify-exit-codes/checks}/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_change "$tmp" "- C1 has a check
- C2 has no check at all"
add_check "$tmp" C1 0

got=$(run_verify "$tmp")
test "$got" = 2 || { echo "expected exit 2 for an unchecked criterion, got $got"; exit 1; }

# C10 must not be satisfied by C1's check: the prefix match requires a separator.
tmp2=$(mktemp -d)
trap 'rm -rf "$tmp" "$tmp2"' EXIT
make_change "$tmp2" "- C1 has a check
- C10 must not match C1's check"
add_check "$tmp2" C1 0

got=$(run_verify "$tmp2")
test "$got" = 2 || { echo "C10 was wrongly bound to C1's check, got exit $got"; exit 1; }
