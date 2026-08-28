#!/bin/sh
# CRITERION: C9 no developer name or identifier appears anywhere in the output
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

grep -qE 'whoami|git log .*%a[en]|git config .*user\.|\$USER\b|\$LOGNAME\b' delta/bin/report && {
    echo "delta/bin/report reads an identity source it should never touch"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 a trivial thing"
add_check "$tmp" fx C1 0
run_verify "$tmp" fx
run_report "$tmp" >/dev/null

html="$tmp/delta/report.html"
# Strip the <style> block first: CSS has its own vocabulary (":root" being
# the obvious one) that coincidentally collides with short usernames like
# "root" itself. The identity question is about the rendered content, not
# about CSS selector names.
body=$(awk '/<style>/{skip=1} /<\/style>/{skip=0; next} !skip' "$html")

whoami_out=$(whoami 2>/dev/null || true)
if [ -n "$whoami_out" ]; then
    printf '%s' "$body" | grep -qw "$whoami_out" && {
        echo "the current user's name leaked into the report"; exit 1; }
fi
printf '%s' "$body" | grep -qE '/home/[a-zA-Z0-9_-]+|/Users/[a-zA-Z0-9_-]+' && {
    echo "an absolute home-directory path leaked into the report"; exit 1; }

exit 0
