#!/bin/sh
# CRITERION: C4 verify exits 3 when a MANUAL criterion has no recorded sign-off
set -eu

DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/example-verify-exit-codes/checks}/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_change "$tmp" "- C1 automated
- C2 MANUAL a human has to look at this"
add_check "$tmp" C1 0

got=$(run_verify "$tmp")
test "$got" = 3 || { echo "expected exit 3 for an unsigned manual criterion, got $got"; exit 1; }

# Recording a sign-off resolves it, and only it.
mkdir -p "$tmp/run"
printf 'C2 signed-off-by: example 2026-08-26 - looked at it\n' > "$tmp/run/signoff.md"

got=$(run_verify "$tmp")
test "$got" = 0 || { echo "expected exit 0 once the manual criterion is signed off, got $got"; exit 1; }
