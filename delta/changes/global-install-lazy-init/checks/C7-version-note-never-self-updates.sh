#!/bin/sh
# CRITERION: C7 verify notes a version mismatch against a newer global runner, and never modifies the repo's own runner
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/global-install-lazy-init/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_fixture_repo "$tmp/repo" 1
fakehome="$tmp/home"; mkdir -p "$fakehome/.delta/bin"
cp "$RUNNER_SRC" "$fakehome/.delta/bin/verify"
sed 's/^RUNNER_VERSION=.*/RUNNER_VERSION=999/' "$fakehome/.delta/bin/verify" > "$fakehome/.delta/bin/verify.new"
mv "$fakehome/.delta/bin/verify.new" "$fakehome/.delta/bin/verify"
chmod +x "$fakehome/.delta/bin/verify"

cp "$tmp/repo/delta/bin/verify" "$tmp/before.copy"
set +e
out=$(cd "$tmp/repo" && HOME="$fakehome" delta/bin/verify fx 2>&1); rc=$?
set -e
test "$rc" -eq 0 || { echo "expected exit 0 (a version note, not a failure), got $rc"; echo "$out"; exit 1; }

printf '%s' "$out" | grep -qi 'runner is v1' || { echo "no version-mismatch note"; echo "$out"; exit 1; }
printf '%s' "$out" | grep -qi 'v999' || { echo "note did not name the global version"; echo "$out"; exit 1; }
cmp -s "$tmp/before.copy" "$tmp/repo/delta/bin/verify" || {
    echo "verify modified its own repo copy of the runner"; exit 1; }
