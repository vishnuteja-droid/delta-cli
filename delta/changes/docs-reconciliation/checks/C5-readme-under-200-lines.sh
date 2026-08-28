#!/bin/sh
# CRITERION: C5 README.md is under 200 lines
set -eu
cd "${DELTA_ROOT:-$PWD}"

lines=$(wc -l < README.md | tr -d ' ')
if [ "$lines" -ge 200 ]; then
    echo "README.md is $lines lines - must be under 200"
    exit 1
fi
exit 0
