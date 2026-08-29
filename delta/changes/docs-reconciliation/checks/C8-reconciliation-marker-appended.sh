#!/bin/sh
# CRITERION: C8 CHANGELOG.md's reconciliation log has a dated entry, and reconciliation is not more than 4 shipped changes overdue
set -eu
cd "${DELTA_ROOT:-$PWD}"

[ -f CHANGELOG.md ] || { echo "CHANGELOG.md does not exist"; exit 1; }
grep -q "^## Reconciliation log" CHANGELOG.md || { echo "CHANGELOG.md has no Reconciliation log section"; exit 1; }

last_marker=$(grep -oE '^### [0-9]{4}-[0-9]{2}-[0-9]{2} —' CHANGELOG.md | tail -1 | awk '{print $2}')
[ -n "$last_marker" ] || { echo "no dated '### YYYY-MM-DD —' reconciliation entry found"; exit 1; }

# This CR's own cadence is "every 2-4 shipped changes" - a marker a change
# or two behind the newest CR entry is normal, not a regression. Only flag
# it once more than 4 CR entries postdate the last reconciliation, which is
# genuinely overdue by the CR's own rule. ISO dates sort correctly as plain
# strings, so this is a straight string comparison, no date arithmetic.
overdue=$(grep -oE '^## CR-[A-Za-z0-9.]+ — .* \([0-9]{4}-[0-9]{2}-[0-9]{2}\)' CHANGELOG.md \
    | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | while IFS= read -r d; do
        later=$(printf '%s\n%s\n' "$last_marker" "$d" | sort | tail -1)
        [ "$later" = "$d" ] && [ "$later" != "$last_marker" ] && echo "$d"
      done | wc -l | tr -d ' ')

if [ "$overdue" -gt 4 ]; then
    echo "$overdue shipped changes have landed since the last reconciliation ($last_marker) - CR-DOCS is overdue"
    exit 1
fi

exit 0
