#!/bin/sh
# CRITERION: C3 charts are inline SVG, no library
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
# Enough runs to make Q1 actually draw a chart, not just report "too short".
mk_change "$tmp" fx "- C1 a trivial thing"
add_check "$tmp" fx C1 0
i=0
while [ "$i" -lt 5 ]; do run_verify "$tmp" fx; i=$((i + 1)); done
run_report "$tmp" >/dev/null

html="$tmp/delta/report.html"
grep -qi '<svg' "$html" || { echo "no inline <svg> found even with 5 runs"; exit 1; }
grep -qiE 'chart\.js|d3\.|highcharts|plotly|cdn\.' "$html" && {
    echo "found a reference to a charting library or CDN"; exit 1; }

exit 0
