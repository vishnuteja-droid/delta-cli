#!/bin/sh
# CRITERION: C6 README.md states the Windows/Git Bash platform requirement and lists the deliberately-not-built items
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0
grep -qi "git bash" README.md || { echo "README.md never mentions the Windows/Git Bash requirement"; fail=1; }
grep -q "^## Deliberately not built" README.md || { echo "README.md has no 'Deliberately not built' section"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
