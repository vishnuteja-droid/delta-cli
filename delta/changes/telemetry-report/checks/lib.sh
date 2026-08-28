# Fixture helpers for the CR-005 checks. No C<n> prefix, so never bound to a
# criterion.

VERIFY_SRC=${VERIFY_SRC:-$PWD/delta/bin/verify}
REPORT_SRC=${REPORT_SRC:-$PWD/delta/bin/report}

# mk_repo <dir>
# An empty but valid repo: .git marker, delta/bin/{verify,report}, no changes.
mk_repo() {
    d=$1
    mkdir -p "$d/.git" "$d/delta/bin" "$d/delta/changes"
    cp "$VERIFY_SRC" "$d/delta/bin/verify"; chmod +x "$d/delta/bin/verify"
    cp "$REPORT_SRC" "$d/delta/bin/report"; chmod +x "$d/delta/bin/report"
}

# mk_change <dir> <change-id> <criteria-block>
mk_change() {
    d=$1/delta/changes/$2
    mkdir -p "$d/checks"
    { printf '# fixture\n\n## Acceptance criteria\n\n'; printf '%s\n' "$3"; } > "$d/spec.md"
}

# add_check <dir> <change-id> <criterion-id> <exit-code>
add_check() {
    f="$1/delta/changes/$2/checks/$3-fixture.sh"
    printf '#!/bin/sh\n# CRITERION: %s fixture\nexit %s\n' "$3" "$4" > "$f"
    chmod +x "$f"
}

run_verify() {  # run_verify <root> <change-id> - never trips set -e on non-zero
    # `|| true` has to sit on the same command as the subshell: set -e fires
    # the instant that command exits non-zero, before any later statement in
    # this function - including a bare `return 0` on the next line - ever runs.
    ( cd "$1" && DELTA_ROOT="$1" delta/bin/verify "$2" >/dev/null 2>&1 ) || true
}

run_report() {  # run_report <root> [extra args...]
    r=$1; shift
    ( cd "$r" && DELTA_ROOT="$r" delta/bin/report "$@" )
}
