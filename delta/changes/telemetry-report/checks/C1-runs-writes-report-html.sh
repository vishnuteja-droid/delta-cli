#!/bin/sh
# CRITERION: C1 report runs with no arguments and writes delta/report.html
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 a trivial thing"
add_check "$tmp" fx C1 0
run_verify "$tmp" fx

set +e
out=$(run_report "$tmp"); rc=$?
set -e
test "$rc" -eq 0 || { echo "report exited $rc, expected 0"; echo "$out"; exit 1; }
test -f "$tmp/delta/report.html" || { echo "delta/report.html was not written"; exit 1; }
test -s "$tmp/delta/report.html" || { echo "delta/report.html is empty"; exit 1; }
