#!/bin/sh
# CRITERION: C11 the palette and glyph vocabulary live in one file; neither the runner nor the HTML generator hardcodes a colour
set -eu

test -f delta/bin/palette.sh || { echo "no delta/bin/palette.sh"; exit 1; }

# Neither verify nor report may define a literal hex colour or raw SGR
# colour-number literal outside of sourcing palette.sh. This is a structural
# check on the source, not on any run's output.
for f in delta/bin/verify delta/bin/report; do
    grep -n '#[0-9a-fA-F]\{6\}' "$f" | grep -v 'palette\.sh' && {
        echo "$f hardcodes a hex colour outside palette.sh"; exit 1; }
    grep -nE "printf '\\\\033\\[3[0-9]m'" "$f" && {
        echo "$f hardcodes a raw SGR escape outside palette.sh"; exit 1; }
done

grep -q '^\. "\$script_dir/palette\.sh"' delta/bin/verify || {
    echo "verify does not source palette.sh"; exit 1; }
grep -q '^\. "\$script_dir/palette\.sh"' delta/bin/report || {
    echo "report does not source palette.sh"; exit 1; }

exit 0
