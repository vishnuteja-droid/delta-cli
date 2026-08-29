#!/bin/sh
# CRITERION: C6 a change can proceed with critique findings outstanding, and they are recorded
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0

# The real invariant: nothing in the gating path (verify's own exit codes,
# archive's gate) reads or cares about critique.md. If either did, a
# critique finding would silently start blocking - exactly what the CR
# rules out. delta/bin/verify and archive.md are unmodified by this CR;
# confirm that holds rather than assuming it.
if grep -q "critique" delta/bin/verify; then
    echo "delta/bin/verify references critique - it must never gate on it"; fail=1
fi
if grep -qi "critique" delta/commands/archive.md; then
    echo "archive.md references critique - it must never gate on it"; fail=1
fi

# Findings have to land somewhere durable (the change folder, not just the
# terminal) for "recorded" to mean anything to a later reader.
grep -q "delta/changes/<id>/critique.md" delta/commands/critique.md \
    || { echo "critique.md does not write findings to the change folder"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
