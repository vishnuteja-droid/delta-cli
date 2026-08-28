#!/bin/sh
# CRITERION: C2 verify runs from the repo copy with no global installation of the runner
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/revert-runner-bootstrap/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/repo"
fakehome="$tmp/home"; mkdir -p "$fakehome"   # no .delta, ever - nothing to install

set +e
out=$(cd "$tmp/repo" && HOME="$fakehome" delta/bin/verify fx 2>&1); rc=$?
set -e
test "$rc" -eq 0 || { echo "expected exit 0 with no global runner anywhere, got $rc"; echo "$out"; exit 1; }
