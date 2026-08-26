#!/bin/sh
# CRITERION: C2 a failing fail-until-fixed check renders as reproduced and the run exits 0
set -eu
DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/reproduction-first-checks/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
bug_fixture "$tmp" "# EXPECT: fail-until-fixed"

got=$(run_verify "$tmp")
test "$got" = 0 || { echo "an outstanding reproduction should exit 0, got $got"; exit 1; }

out=$(run_verify_out "$tmp")
printf '%s' "$out" | grep -q 'reproduced' || {
    echo "the run did not render the check as reproduced"; exit 1; }
printf '%s' "$out" | grep -qi 'failed' && {
    echo "a reproduction was counted as a failure"; exit 1; }

# It is counted separately, not as a pass.
printf '%s' "$out" | grep -q '1 reproduced' || {
    echo "the summary did not count the reproduction separately"; exit 1; }
printf '%s' "$out" | grep -q '0 passed' || {
    echo "a reproduction was counted as passed"; exit 1; }

# And the reproduction is recorded with a timestamp.
grep -qE '^B1 reproduced: [0-9]{4}-[0-9]{2}-[0-9]{2}T' "$tmp/run/reproductions.md" || {
    echo "no timestamped reproduction record was written"; exit 1; }
