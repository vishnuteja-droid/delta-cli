# Fixture helpers for the CR-004 checks. No C<n> prefix, so never bound to a
# criterion.

VERIFY_SRC=${VERIFY_SRC:-$PWD/delta/bin/verify}
PALETTE_SRC=${PALETTE_SRC:-$PWD/delta/bin/palette.sh}
REPORT_SRC=${REPORT_SRC:-$PWD/delta/bin/report}

# mk_repo <dir>
mk_repo() {
    d=$1
    mkdir -p "$d/.git" "$d/delta/bin" "$d/delta/changes"
    cp "$VERIFY_SRC" "$d/delta/bin/verify"; chmod +x "$d/delta/bin/verify"
    cp "$PALETTE_SRC" "$d/delta/bin/palette.sh"
}

# mk_change <dir> <change-id> <criteria-block>
mk_change() {
    d=$1/delta/changes/$2
    mkdir -p "$d/checks"
    { printf '# fixture\n\n## Acceptance criteria\n\n'; printf '%s\n' "$3"; } > "$d/spec.md"
}

# add_check <dir> <change-id> <criterion-id> <body-after-shebang>
add_check() {
    f="$1/delta/changes/$2/checks/$3-fixture.sh"
    { printf '#!/bin/sh\n# CRITERION: %s fixture\n' "$3"; printf '%s\n' "$4"; } > "$f"
    chmod +x "$f"
}

have_script() { command -v script >/dev/null 2>&1; }
have_python3() { command -v python3 >/dev/null 2>&1; }
