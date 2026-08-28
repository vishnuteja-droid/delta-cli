#!/bin/sh
# CRITERION: C5 checks that could not run are counted separately from criteria that failed
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 passes
- C2 cannot run at all"
add_check "$tmp" fx C1 0
# C2's check exists but is never chmod +x'd - an error, not a failure.
f="$tmp/delta/changes/fx/checks/C2-fixture.sh"
printf '#!/bin/sh\n# CRITERION: C2 fixture\nexit 0\n' > "$f"
run_verify "$tmp" fx
run_report "$tmp" >/dev/null

html="$tmp/delta/report.html"
# The headline must count only the ordinary pass/fail pair (1 of 1) - the
# unrunnable check must not appear as a failure inside it.
grep -q 'checks failed on verify (0 of 1)' "$html" || {
    echo "the error-state check leaked into the pass/fail headline"
    grep 'checks failed on verify' "$html" || true
    exit 1
}
grep -q '1 error (could not run at all' "$html" || {
    echo "the error-state check was not reported as its own separate count"; exit 1; }
