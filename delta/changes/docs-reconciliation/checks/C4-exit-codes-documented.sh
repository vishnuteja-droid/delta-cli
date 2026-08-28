#!/bin/sh
# CRITERION: C4 every exit code delta/bin/verify's source can actually produce is documented in both README.md and delta/commands/verify.md, and neither documents a code the source doesn't produce
set -eu
cd "${DELTA_ROOT:-$PWD}"

source_codes=$(grep -oE "exit [0-9]+" delta/bin/verify | awk '{print $2}' | sort -un)

fail=0
for n in $source_codes; do
    grep -qE "\*\*${n}\*\*" README.md || { echo "README.md never documents exit $n"; fail=1; }
    grep -qE "\*\*${n}\*\*" delta/commands/verify.md || { echo "delta/commands/verify.md never documents exit $n"; fail=1; }
done

# The reverse direction: a bolded number in either doc's exit-code table
# that the source can't actually produce is an invented code.
readme_codes=$(awk '/^\| \*\*[0-9]+\*\* \|/' README.md | grep -oE '\*\*[0-9]+\*\*' | tr -d '*' | sort -un)
verifymd_codes=$(awk '/^- \*\*[0-9]+\*\*/' delta/commands/verify.md | grep -oE '\*\*[0-9]+\*\*' | tr -d '*' | sort -un)

for n in $readme_codes; do
    echo "$source_codes" | grep -qx "$n" || { echo "README.md documents exit $n but delta/bin/verify never exits with it"; fail=1; }
done
for n in $verifymd_codes; do
    echo "$source_codes" | grep -qx "$n" || { echo "delta/commands/verify.md documents exit $n but delta/bin/verify never exits with it"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
