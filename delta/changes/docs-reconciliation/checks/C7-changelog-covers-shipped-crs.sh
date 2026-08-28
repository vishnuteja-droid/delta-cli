#!/bin/sh
# CRITERION: C7 CHANGELOG.md has one entry for every shipped change in git log between the initial Add delta commit and this change, each with a behaviour change: line
set -eu
cd "${DELTA_ROOT:-$PWD}"

[ -f CHANGELOG.md ] || { echo "CHANGELOG.md does not exist"; exit 1; }

fail=0
for spec in delta/changes/*/spec.md; do
    title=$(head -1 "$spec")
    case "$title" in
        "# CR-"*" — "*) ;;
        *) continue ;;  # not a CR-labelled change (e.g. the worked example) - nothing to reconcile
    esac
    crid=${title#"# "}
    crid=${crid%% — *}
    [ "$crid" = "CR-DOCS" ] && continue  # tracked via the reconciliation log, not a per-CR entry

    section=$(awk -v id="## $crid " 'index($0, id) == 1 {f=1; print; next} f && /^## /{exit} f' CHANGELOG.md)
    [ -n "$section" ] || { echo "CHANGELOG.md has no entry for $crid (from $spec)"; fail=1; continue; }
    printf '%s\n' "$section" | grep -qi "^behaviour change:" || {
        echo "CHANGELOG.md's $crid entry has no 'behaviour change:' line"; fail=1; }
done

[ "$fail" -eq 0 ] && exit 0
exit 1
