#!/bin/sh
# CRITERION: C3 a fresh clone of a delta-using repo runs verify with no setup
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/revert-runner-bootstrap/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/origin"

# `git clone` stands in for a teammate cloning: a plain file copy of a
# committed tree is the same thing this criterion cares about - a copy of
# the repo, nothing machine-specific carried over.
cp -r "$tmp/origin" "$tmp/clone"
fakehome="$tmp/home"; mkdir -p "$fakehome"

set +e
out=$(cd "$tmp/clone" && HOME="$fakehome" delta/bin/verify fx 2>&1); rc=$?
set -e
test "$rc" -eq 0 || { echo "fresh clone did not run cleanly, got exit $rc"; echo "$out"; exit 1; }
