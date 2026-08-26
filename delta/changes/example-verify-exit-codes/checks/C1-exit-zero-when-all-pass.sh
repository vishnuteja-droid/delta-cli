#!/bin/sh
# CRITERION: C1 verify exits 0 when every criterion has a passing check
set -eu

DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/example-verify-exit-codes/checks}/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_change "$tmp" "- C1 first thing
- C2 second thing"
add_check "$tmp" C1 0
add_check "$tmp" C2 0

got=$(run_verify "$tmp")
test "$got" = 0 || { echo "expected exit 0, got $got"; exit 1; }
