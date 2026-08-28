#!/bin/sh
# CRITERION: C12 the same run rendered in the terminal and in the HTML report uses identical state colours
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"

# Both sides derive from the same RGB triples in palette.sh - confirm the
# derivation actually agrees, rather than trusting two independent formulas
# to coincidentally produce the same bytes.
. delta/bin/palette.sh
accent_hex=$(palette_hex "$PALETTE_RGB_ACCENT")
pass_hex=$(palette_hex "$PALETTE_RGB_PASS")
fail_hex=$(palette_hex "$PALETTE_RGB_FAIL")

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
cp "$REPORT_SRC" "$tmp/repo/delta/bin/report"; chmod +x "$tmp/repo/delta/bin/report"
mk_change "$tmp/repo" fx "- C1 a thing"
add_check "$tmp/repo" fx C1 'exit 0'
( cd "$tmp/repo" && delta/bin/verify fx >/dev/null 2>&1 )
( cd "$tmp/repo" && delta/bin/report >/dev/null )

html="$tmp/repo/delta/report.html"
grep -q -- "--accent: $accent_hex" "$html" || { echo "report's accent hex does not match verify's palette"; exit 1; }
grep -q -- "--pass: $pass_hex" "$html" || { echo "report's pass hex does not match verify's palette"; exit 1; }
grep -q -- "--fail: $fail_hex" "$html" || { echo "report's fail hex does not match verify's palette"; exit 1; }

exit 0
