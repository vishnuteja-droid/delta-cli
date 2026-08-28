#!/bin/sh
# CRITERION: C4 MANUAL criteria are counted separately and never as failures
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 automated
- C2 MANUAL judged by a human
      reason: no assertable output
      look at: nothing"
add_check "$tmp" fx C1 0
mkdir -p "$tmp/delta/changes/fx/run"
printf 'C2 signed-off-by: build 2026-08-28 - looked at it\n' > "$tmp/delta/changes/fx/run/signoff.md"
run_verify "$tmp" fx
run_report "$tmp" >/dev/null

html="$tmp/delta/report.html"
# The headline Q2 number must read 0 failed out of 1 (the automated
# criterion only) - the MANUAL one must never inflate either side of it.
grep -q 'checks failed on verify (0 of 1)' "$html" || {
    echo "MANUAL criterion leaked into the headline failure count"
    grep 'checks failed on verify' "$html" || true
    exit 1
}
grep -q '1 MANUAL (not automatable' "$html" || {
    echo "MANUAL was not reported as its own separate count"; exit 1; }
