#!/bin/sh
# CRITERION: C5 a passing fail-until-fixed check on a fresh spec is reported as suspicious
#
# The case worth getting right. A reproduction that passes immediately never
# captured the bug, and accepting it silently would let someone "fix" a bug
# they never reproduced.
set -eu
DELTA_VERIFY=${DELTA_VERIFY:-delta/bin/verify}
. "${DELTA_CHECK_DIR:-delta/changes/reproduction-first-checks/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
bug_fixture "$tmp" "# EXPECT: fail-until-fixed"
touch "$tmp/FIXED"          # it passes on the very first run: fresh spec, no history

got=$(run_verify "$tmp")
test "$got" = 6 || { echo "a repro that does not reproduce should exit 6, got $got"; exit 1; }

out=$(run_verify_out "$tmp")
printf '%s' "$out" | grep -q 'suspicious' || {
    echo "it was not reported as suspicious"; exit 1; }

# It must NOT be silently treated as a fix. Written as an explicit if: the
# shorthand `A || B && C` parses as `(A || B) && C`, which is not this test.
if [ -f "$tmp/run/reproductions.md" ] \
   && grep -q '^B1 fixed:' "$tmp/run/reproductions.md"; then
    echo "a repro that never reproduced was recorded as fixed"; exit 1
fi
grep -q '^# EXPECT: fail-until-fixed' "$tmp/checks/B1-repro.sh" || {
    echo "a repro that never reproduced was flipped to a normal check"; exit 1; }
