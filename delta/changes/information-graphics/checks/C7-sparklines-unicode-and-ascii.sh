#!/bin/sh
# CRITERION: C7 sparklines use only Unicode block characters and fall back to a numeric summary in ASCII mode
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 something'
add_check "$tmp/repo" x C1 'exit 0'
run_recorded "$tmp/repo" x
run_recorded "$tmp/repo" x

unicode_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify --all)
printf '%s\n' "$unicode_out" | grep -q '[▁▂▃▄▅▆▇█]' \
    || { echo "unicode mode: no block-character sparkline found"; exit 1; }

ascii_out=$(cd "$tmp/repo" && LANG=C ./delta/bin/verify --all)
printf '%s\n' "$ascii_out" | grep -q '[▁▂▃▄▅▆▇█]' \
    && { echo "ASCII mode still emitted a Unicode sparkline character"; exit 1; }
printf '%s\n' "$ascii_out" | grep -qE '[0-9]+/[0-9]+ [0-9.]+s avg' \
    || { echo "ASCII mode did not fall back to a numeric summary"; exit 1; }

exit 0
