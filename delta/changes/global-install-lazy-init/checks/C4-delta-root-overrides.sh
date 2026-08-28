#!/bin/sh
# CRITERION: C4 DELTA_ROOT overrides discovery
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/global-install-lazy-init/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/target"
elsewhere="$tmp/elsewhere"; mkdir -p "$elsewhere"

# set +e around each capture: both invocations legitimately exit non-zero on
# one branch, and $() inside a plain assignment is not a conditional context
# - set -e would otherwise kill the script right here, before rc is even read.
set +e
out=$(cd "$elsewhere" && DELTA_ROOT="$tmp/target" "$tmp/target/delta/bin/verify" fx 2>&1); rc=$?
set -e
test "$rc" -eq 0 || { echo "expected exit 0 against the DELTA_ROOT target, got $rc"; echo "$out"; exit 1; }
printf '%s' "$out" | grep -q "root: $tmp/target" || {
    echo "did not announce the DELTA_ROOT-resolved root"; echo "$out"; exit 1; }

set +e
out2=$(cd "$elsewhere" && DELTA_ROOT="$elsewhere" "$tmp/target/delta/bin/verify" fx 2>&1); rc2=$?
set -e
test "$rc2" -eq 4 || { echo "expected exit 4 for a DELTA_ROOT with no delta/, got $rc2"; echo "$out2"; exit 1; }
