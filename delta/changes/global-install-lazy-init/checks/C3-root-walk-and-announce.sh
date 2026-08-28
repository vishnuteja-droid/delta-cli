#!/bin/sh
# CRITERION: C3 delta/bin/verify resolves its root by walking up for delta/, and announces the root when it differs from cwd
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/global-install-lazy-init/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/repo"

nested="$tmp/repo/some/deep/nested/dir"; mkdir -p "$nested"
out=$(cd "$nested" && "$tmp/repo/delta/bin/verify" fx 2>&1) || true
printf '%s' "$out" | grep -q "root: $tmp/repo" || {
    echo "no root announcement when invoked with cwd nested under the root"; echo "$out"; exit 1; }

out2=$(cd "$tmp/repo" && delta/bin/verify fx 2>&1) || true
if printf '%s' "$out2" | grep -q '^  root:'; then
    echo "root was announced even though cwd already equals the resolved root"; echo "$out2"; exit 1
fi
