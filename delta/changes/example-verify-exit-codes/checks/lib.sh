# Shared fixture helper for the example checks. Not itself a check: it has no
# C<n> filename prefix, so verify never binds it to a criterion.

# make_change <dir> <criteria-block>
# Writes a minimal but valid spec.md into <dir>.
make_change() {
    mkdir -p "$1/checks"
    {
        printf '# fixture\n\n## Acceptance criteria\n\n'
        printf '%s\n' "$2"
    } > "$1/spec.md"
}

# add_check <dir> <id> <exit-code>
add_check() {
    printf '#!/bin/sh\n# CRITERION: %s fixture\nexit %s\n' "$2" "$3" > "$1/checks/$2-fixture.sh"
    chmod +x "$1/checks/$2-fixture.sh"
}

# run_verify <dir> -> echoes the exit code, discards the output
# Written as an if-statement rather than a bare call so that a non-zero exit
# does not trip `set -e` in the calling check before the code is captured.
run_verify() {
    if "$DELTA_VERIFY" "$1" >/dev/null 2>&1; then _rc=0; else _rc=$?; fi
    printf '%s' "$_rc"
}
