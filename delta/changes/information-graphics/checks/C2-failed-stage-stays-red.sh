#!/bin/sh
# CRITERION: C2 a failed stage stays red on subsequent commands until resolved
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"
have_script || { echo "no script(1) on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 something'
add_check "$tmp/repo" x C1 'exit 1'

# The rail is drawn before this run's own run/ directory is created, so it
# reflects the *previous* run's outcome - "where am I before you ask" means
# before this command's own work, not before the command that already
# happened. Run 1 (the very first ever) has no prior run to report, so it
# correctly shows pending; runs 2 and 3 must both show C1's failure as red,
# proving the stage stays red across repeated commands, not just the one
# right after it failed.
(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "./delta/bin/verify x" /dev/null </dev/null >/dev/null 2>&1) || true
run2=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "./delta/bin/verify x" /dev/null </dev/null 2>/dev/null) || true
run3=$(cd "$tmp/repo" && LANG=en_US.UTF-8 script -qec "./delta/bin/verify x" /dev/null </dev/null 2>/dev/null) || true

for run in "$run2" "$run3"; do
    dotsline=$(printf '%s' "$run" | grep -aE '●|○' | head -1)
    [ -n "$dotsline" ] || { echo "no rail dots line found"; exit 1; }
    printf '%s' "$dotsline" | grep -qP '\x1b\[31m\xe2\x97\x8f' 2>/dev/null \
        || { echo "verify stage is not red after a failing run"; exit 1; }
done

exit 0
