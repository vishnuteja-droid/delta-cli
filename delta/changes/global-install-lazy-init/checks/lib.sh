# Fixture helpers for the CR-002 checks. No C<n> prefix, so never bound to a
# criterion.

RUNNER_SRC=${RUNNER_SRC:-$PWD/delta/bin/verify}

# mk_fixture_repo <dir> [runner-version]
# A minimal but complete "repo": a .git marker, delta/{truth,changes,bin/verify},
# and one change `fx` with a single passing criterion C1. Stands in for a repo
# that already ran propose once.
mk_fixture_repo() {
    d=$1; ver=${2:-}
    mkdir -p "$d/.git" "$d/delta/truth" "$d/delta/bin" "$d/delta/changes/fx/checks"
    cp "$RUNNER_SRC" "$d/delta/bin/verify"
    if [ -n "$ver" ]; then
        sed "s/^RUNNER_VERSION=.*/RUNNER_VERSION=$ver/" "$d/delta/bin/verify" > "$d/delta/bin/verify.new"
        mv "$d/delta/bin/verify.new" "$d/delta/bin/verify"
    fi
    chmod +x "$d/delta/bin/verify"
    printf '# fixture\n\n## Acceptance criteria\n\n- C1 a trivial thing\n' > "$d/delta/changes/fx/spec.md"
    printf '#!/bin/sh\n# CRITERION: C1 a trivial thing\nexit 0\n' > "$d/delta/changes/fx/checks/C1-trivial.sh"
    chmod +x "$d/delta/changes/fx/checks/C1-trivial.sh"
}
