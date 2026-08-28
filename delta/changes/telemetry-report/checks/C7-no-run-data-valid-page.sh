#!/bin/sh
# CRITERION: C7 a repo with no run/ data at all produces a valid page explaining that, not an error
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"    # no changes, no runs, at all

set +e
out=$(run_report "$tmp"); rc=$?
set -e
test "$rc" -eq 0 || { echo "expected exit 0 on empty history, got $rc"; echo "$out"; exit 1; }
html="$tmp/delta/report.html"
test -f "$html" || { echo "no report.html written"; exit 1; }
grep -qi '<!doctype html>' "$html" || { echo "not a valid HTML document"; exit 1; }
grep -qi 'No.*run.*data yet' "$html" || { echo "did not explain the empty state"; exit 1; }

# Also with changes present but none ever verified.
mk_change "$tmp" fx "- C1 never run"
set +e
out2=$(run_report "$tmp"); rc2=$?
set -e
test "$rc2" -eq 0 || { echo "expected exit 0 with an unverified change, got $rc2"; exit 1; }
grep -qi 'No.*run.*data yet' "$tmp/delta/report.html" || {
    echo "did not explain the empty state once a change exists but was never verified"; exit 1; }
