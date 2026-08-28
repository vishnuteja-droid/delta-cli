#!/bin/sh
# CRITERION: C8 CHANGELOG.md's reconciliation log ends with a dated entry that is not older than the CR entries it reconciles
set -eu
cd "${DELTA_ROOT:-$PWD}"

[ -f CHANGELOG.md ] || { echo "CHANGELOG.md does not exist"; exit 1; }
grep -q "^## Reconciliation log" CHANGELOG.md || { echo "CHANGELOG.md has no Reconciliation log section"; exit 1; }

last_marker=$(grep -oE '^### [0-9]{4}-[0-9]{2}-[0-9]{2} —' CHANGELOG.md | tail -1 | awk '{print $2}')
[ -n "$last_marker" ] || { echo "no dated '### YYYY-MM-DD —' reconciliation entry found"; exit 1; }

# ISO dates sort correctly as plain strings, so the newest CR heading date is
# the last one after a lexical sort - no date arithmetic needed.
newest_cr=$(grep -oE '^## CR-[A-Za-z0-9.]+ — .* \([0-9]{4}-[0-9]{2}-[0-9]{2}\)' CHANGELOG.md \
    | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | sort | tail -1)

if [ -n "$newest_cr" ]; then
    later=$(printf '%s\n%s\n' "$last_marker" "$newest_cr" | sort | tail -1)
    if [ "$later" != "$last_marker" ]; then
        echo "reconciliation marker ($last_marker) is older than the newest CR entry it should cover ($newest_cr)"
        exit 1
    fi
fi

exit 0
