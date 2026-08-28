#!/bin/sh
# CRITERION: C2 the report opens over file:// with zero network requests and no JavaScript
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/telemetry-report/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp"
mk_change "$tmp" fx "- C1 a trivial thing"
add_check "$tmp" fx C1 0
run_verify "$tmp" fx
run_report "$tmp" >/dev/null

html="$tmp/delta/report.html"
grep -qi '<script' "$html" && { echo "found a <script> tag - report must have no JavaScript"; exit 1; }
grep -qiE 'https?://' "$html" && { echo "found an http(s):// reference - report must make zero network requests"; exit 1; }
grep -qi '<link[^>]*href' "$html" && { echo "found a <link href> - external stylesheet/font would be a network request"; exit 1; }
grep -qi '@import' "$html" && { echo "found @import - would be a network request"; exit 1; }
grep -qi '<!doctype html>' "$html" || { echo "not a valid standalone HTML document"; exit 1; }
