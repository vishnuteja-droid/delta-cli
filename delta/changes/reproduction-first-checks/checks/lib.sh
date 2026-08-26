# Fixture helper for the CR-001 checks. No C<n> prefix, so it is never bound
# to a criterion.

# bug_fixture <dir> <expect-line>
# A change whose single criterion B1 is a reproduction. The check passes only
# once <dir>/FIXED exists, standing in for "the fix landed".
bug_fixture() {
    mkdir -p "$1/checks"
    printf '# fixture\n\n## Acceptance criteria\n\n- B1 the bug happens\n' > "$1/spec.md"
    {
        printf '#!/bin/sh\n'
        printf '# CRITERION: B1 the bug happens\n'
        [ -n "$2" ] && printf '%s\n' "$2"
        printf 'test -f "$DELTA_CHANGE_DIR/FIXED"\n'
    } > "$1/checks/B1-repro.sh"
    chmod +x "$1/checks/B1-repro.sh"
}

# Non-zero exits must not trip `set -e` before the code is captured.
run_verify() {
    if "$DELTA_VERIFY" "$@" >/dev/null 2>&1; then _rc=0; else _rc=$?; fi
    printf '%s' "$_rc"
}
run_verify_out() { "$DELTA_VERIFY" "$@" 2>&1 || true; }
