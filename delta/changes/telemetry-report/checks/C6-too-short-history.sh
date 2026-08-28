#!/bin/sh
# CRITERION: C6 a repo with fewer than a handful of runs says the history is too short rather than charting it
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 a trivial thing"
add_check "$tmp" fx C1 0
run_verify "$tmp" fx   # exactly one run

run_report "$tmp" >/dev/null
html="$tmp/delta/report.html"

grep -q 'Not enough history yet to chart a trend' "$html" || {
    echo "did not say the history was too short at 1 run"; exit 1; }
# And it must not have drawn a populated day-count bar chart for Q1 instead.
awk '/aria-label="verify runs per day"/,/<\/svg>/' "$html" | grep -q '<rect' && {
    echo "drew a Q1 chart anyway despite too little history"; exit 1; }

exit 0
