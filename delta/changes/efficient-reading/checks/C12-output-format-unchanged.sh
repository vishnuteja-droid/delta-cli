#!/bin/sh
# CRITERION: C12 the findings file's four headings and the spec's four sections plus Acceptance criteria are unchanged from before this CR
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0

# explore's four findings headings - "Entry points" and "Data touched" and
# "Unknowns" are literal section headers; "Call chain" is referenced as a
# criterion 2 label even though CR-006 turned it into a diagram - the
# *concept* (what calls what down to the state it touches) has to survive.
grep -q "^1\. \*\*Entry points\.\*\*" delta/commands/explore.md \
    || { echo "explore.md no longer lists 'Entry points' as finding #1"; fail=1; }
grep -q "^2\. \*\*Call chain\.\*\*" delta/commands/explore.md \
    || { echo "explore.md no longer lists 'Call chain' as finding #2"; fail=1; }
grep -q "^3\. \*\*Data touched\.\*\*" delta/commands/explore.md \
    || { echo "explore.md no longer lists 'Data touched' as finding #3"; fail=1; }
grep -q "^4\. \*\*Unknowns\.\*\*" delta/commands/explore.md \
    || { echo "explore.md no longer lists 'Unknowns' as finding #4"; fail=1; }

# propose's spec format - the four delta sections plus Acceptance criteria.
for section in ADDED MODIFIED REMOVED RENAMED; do
    grep -qE "^- \*\*$section\*\*" delta/commands/propose.md \
        || { echo "propose.md no longer defines the $section spec section"; fail=1; }
done
grep -q '## Acceptance criteria' delta/commands/propose.md \
    || { echo "propose.md no longer has an Acceptance criteria heading"; fail=1; }

# The checkbox convention apply.md relies on, and the archived: marker
# archive.md writes, both have to survive too - CR-007 touches neither.
grep -q -- '- \[x\]' delta/commands/apply.md \
    || { echo "apply.md no longer documents the - [x] checkbox convention"; fail=1; }
grep -q 'archived: <date>' delta/commands/archive.md \
    || { echo "archive.md no longer documents the archived: <date> marker"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
