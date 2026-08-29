#!/bin/sh
# CRITERION: C10 every graphic here degrades to plain text when piped, with no escape sequences in the output
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 something'
add_check "$tmp/repo" x C1 'exit 0'
run_recorded "$tmp/repo" x

esc=$(printf '\033')
fail=0

verify_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify x 2>&1)
printf '%s' "$verify_out" | grep -qF "$esc" && { echo "verify's piped output has an escape sequence"; fail=1; }

all_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify --all 2>&1)
printf '%s' "$all_out" | grep -qF "$esc" && { echo "verify --all's piped output has an escape sequence"; fail=1; }

rail_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/stage-rail verify x 2>&1)
printf '%s' "$rail_out" | grep -qF "$esc" && { echo "stage-rail's piped output has an escape sequence"; fail=1; }

[ "$fail" -eq 0 ] && exit 0
exit 1
