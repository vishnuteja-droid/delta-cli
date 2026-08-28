#!/bin/sh
# CRITERION: C2 every path shown in README.md's two Layout trees exists in the repo
set -eu
cd "${DELTA_ROOT:-$PWD}"

tmp=$(mktemp); trap 'rm -f "$tmp"' EXIT

awk '
    /^## Layout/{sect=1}
    sect && /^## / && !/^## Layout/{sect=0}
    sect && /^```/{infence=!infence; next}
    sect && infence {print}
' README.md > "$tmp"

[ -s "$tmp" ] || { echo "no fenced tree found under ## Layout in README.md"; exit 1; }

sed 's/^[ \t]*//' "$tmp" | awk -F'  +' '{print $1}' > "$tmp.paths"

while IFS= read -r p; do
    [ -n "$p" ] || continue
    case "$p" in
        ~*)
            # machine-relative (under $HOME) - not a repo path; delta/bin/install
            # actually writing these is C3's job, not this one.
            continue ;;
        delta/*)
            resolved=$p ;;
        changes/*)
            rest=${p#changes/*/}
            resolved="delta/changes/*/${rest}" ;;
        *)
            resolved="delta/$p" ;;
    esac
    case "$resolved" in
        *'*'*)
            set -- $resolved
            [ -e "$1" ] || { echo "no match for '$p' (expected under $resolved)"; exit 1; } ;;
        *)
            [ -e "$resolved" ] || { echo "'$p' (expected at $resolved) does not exist"; exit 1; } ;;
    esac
done < "$tmp.paths"
rm -f "$tmp.paths"

exit 0
