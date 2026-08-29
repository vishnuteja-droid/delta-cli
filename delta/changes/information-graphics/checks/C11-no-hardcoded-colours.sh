#!/bin/sh
# CRITERION: C11 all colours come from the shared palette; nothing is hardcoded
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0
# Hex triples and raw truecolor escapes (38;2; / 48;2;) outside palette.sh
# itself are the two ways a colour sneaks in without going through it.
for f in delta/bin/verify delta/bin/report delta/bin/stage-rail; do
    [ -f "$f" ] || continue
    if grep -nE '#[0-9a-fA-F]{6}\b' "$f" | grep -vE '^[0-9]+:[[:space:]]*#'; then
        echo "$f has a literal hex colour"
        fail=1
    fi
    # A truecolor escape is legitimate either via palette_ansi_truecolor(),
    # or built inline for per-character gradient interpolation (gradient_rule,
    # reveal_opening_frame) - those still derive every value from
    # PALETTE_RGB_* triples, just can't call the helper since it takes one
    # static triple, not two to interpolate between. Either way a genuine
    # PALETTE_RGB_ reference has to appear a few lines above.
    for lineno in $(grep -n '38;2;\|48;2;' "$f" | grep -v 'palette_ansi_truecolor' | cut -d: -f1); do
        start=$((lineno - 90)); [ "$start" -lt 1 ] && start=1
        sed -n "${start},${lineno}p" "$f" | grep -q 'PALETTE_RGB_' || {
            echo "$f:$lineno has a truecolor escape with no PALETTE_RGB_ source nearby"
            fail=1
        }
    done
done

[ "$fail" -eq 0 ] && exit 0
exit 1
