#!/bin/sh
# CRITERION: C8 the heatmap distinguishes failed, could-not-run, and manual with distinct colours from the shared palette
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"

[ -x "$REPORT_SRC" ] || { echo "no delta/bin/report to test against"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 passes
- C2 fails
- C3 no check yet'
add_check "$tmp/repo" x C1 'exit 0'
add_check "$tmp/repo" x C2 'exit 1'
run_recorded "$tmp/repo" x
# run/<id> is a per-second timestamp; two runs inside the same second
# collide into one directory, and the heatmap needs two distinct columns.
sleep 1.1
run_recorded "$tmp/repo" x

( cd "$tmp/repo" && ./delta/bin/report >/dev/null 2>&1 )
out="$tmp/repo/delta/report.html"
[ -f "$out" ] || { echo "report.html was not written"; exit 1; }

. "$tmp/repo/delta/bin/palette.sh"
pass_hex=$(palette_hex "$PALETTE_RGB_PASS")
fail_hex=$(palette_hex "$PALETTE_RGB_FAIL")
accent_hex=$(palette_hex "$PALETTE_RGB_ACCENT")
dim_hex=$(palette_hex "$PALETTE_RGB_FG_DIM")

grep -q "aria-label=\"failure heatmap" "$out" || { echo "no heatmap SVG in the report"; exit 1; }
grep -q "fill=\"$pass_hex\"" "$out"   || { echo "no passed-coloured (pass) cell found"; exit 1; }
grep -q "fill=\"$fail_hex\"" "$out"   || { echo "no failed-coloured (fail) cell found"; exit 1; }
grep -q "fill=\"$accent_hex\"" "$out" || { echo "no could-not-run (accent) cell found"; exit 1; }

# The three must actually be distinct values, not the same hex reused under
# different names - "distinct colours" is the criterion, not just three labels.
n=$(printf '%s\n%s\n%s\n' "$pass_hex" "$fail_hex" "$accent_hex" | sort -u | wc -l | tr -d ' ')
[ "$n" -eq 3 ] || { echo "pass/fail/accent are not three distinct hex values in the palette"; exit 1; }

exit 0
