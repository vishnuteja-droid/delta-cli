# Fixture helpers for the CR-006 checks. No C<n> prefix, so never bound to a
# criterion.

VERIFY_SRC=${VERIFY_SRC:-$PWD/delta/bin/verify}
PALETTE_SRC=${PALETTE_SRC:-$PWD/delta/bin/palette.sh}
REPORT_SRC=${REPORT_SRC:-$PWD/delta/bin/report}
STAGE_RAIL_SRC=${STAGE_RAIL_SRC:-$PWD/delta/bin/stage-rail}

# mk_repo <dir>
mk_repo() {
    d=$1
    mkdir -p "$d/.git" "$d/delta/bin" "$d/delta/changes"
    cp "$VERIFY_SRC" "$d/delta/bin/verify"; chmod +x "$d/delta/bin/verify"
    cp "$PALETTE_SRC" "$d/delta/bin/palette.sh"
    cp "$STAGE_RAIL_SRC" "$d/delta/bin/stage-rail"; chmod +x "$d/delta/bin/stage-rail"
    [ -f "$REPORT_SRC" ] && { cp "$REPORT_SRC" "$d/delta/bin/report"; chmod +x "$d/delta/bin/report"; }
}

# mk_change <dir> <change-id> <criteria-block>
mk_change() {
    d=$1/delta/changes/$2
    mkdir -p "$d/checks"
    { printf '# fixture\n\n## ADDED\n\n- item one\n- item two\n\n## Acceptance criteria\n\n'; printf '%s\n' "$3"; } > "$d/spec.md"
}

# add_check <dir> <change-id> <criterion-id> <body-after-shebang>
add_check() {
    f="$1/delta/changes/$2/checks/$3-fixture.sh"
    { printf '#!/bin/sh\n# CRITERION: %s fixture\n' "$3"; printf '%s\n' "$4"; } > "$f"
    chmod +x "$f"
}

# run_recorded <dir> <change-id> - runs verify for real so a genuine
# run/<ts>/results.tsv exists, rather than hand-writing one: sparklines and
# the heatmap both read real run history, so the fixture has to produce some.
# A fixture is often deliberately built to fail a check, so this never
# propagates verify's own exit code to the caller.
run_recorded() {
    ( cd "$1" && ./delta/bin/verify "$2" >/dev/null 2>&1 ) || true
}

have_script() { command -v script >/dev/null 2>&1; }
have_python3() { command -v python3 >/dev/null 2>&1; }
