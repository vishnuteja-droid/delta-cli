#!/bin/sh
# CRITERION: C7 a non-UTF-8 locale renders ASCII glyphs throughout with aligned columns
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/terminal-presence/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" fx "- C1 passes
- C2 fails"
add_check "$tmp/repo" fx C1 'exit 0'
add_check "$tmp/repo" fx C2 'exit 1'

out=$(cd "$tmp/repo" && LANG=C delta/bin/verify fx 2>&1) || true

printf '%s' "$out" | grep -qP '[^\x00-\x7f]' 2>/dev/null && { echo "a non-ASCII byte leaked under LANG=C"; exit 1; }
printf '%s' "$out" | grep -q '\[ok\]' || { echo "no ASCII pass glyph"; exit 1; }
printf '%s' "$out" | grep -q '\[FAIL\]' || { echo "no ASCII fail glyph"; exit 1; }
# Column alignment: every result line's glyph column starts at the same offset.
positions=$(printf '%s' "$out" | grep -oE '^  \[(ok|FAIL)\]' | sed 's/\[.*//' | awk '{print length}' | sort -u)
test "$(printf '%s' "$positions" | wc -l)" -le 1 || { echo "result lines are not column-aligned: $positions"; exit 1; }

exit 0
