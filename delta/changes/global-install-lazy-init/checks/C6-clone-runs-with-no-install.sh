#!/bin/sh
# CRITERION: C6 a repo with delta/bin/verify committed runs correctly with no ~/.delta present
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/global-install-lazy-init/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/repo"
emptyhome="$tmp/home"; mkdir -p "$emptyhome"     # exists, but no .delta inside it - never installed

set +e
out=$(cd "$tmp/repo" && HOME="$emptyhome" delta/bin/verify fx 2>&1); rc=$?
set -e
test "$rc" -eq 0 || { echo "expected exit 0 with no install present, got $rc"; echo "$out"; exit 1; }
if printf '%s' "$out" | grep -qi 'no such file\|cannot\|error'; then
    echo "verify errored over the absent ~/.delta instead of silently skipping the version note"
    echo "$out"; exit 1
fi
