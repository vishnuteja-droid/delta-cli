#!/bin/sh
# CRITERION: C1 the lifecycle rail appears on every command and shows the correct stage
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 something'
add_check "$tmp/repo" x C1 'exit 0'

# The rail is drawn before this run's own run/ directory exists, so it
# reports the *previous* run's outcome (see the comment in delta/bin/verify
# itself). Run once to create that history, then check the second run's rail.
cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify x >/dev/null 2>&1
out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify x 2>&1)

rail=$(printf '%s\n' "$out" | grep -A1 '^ *●━\|^ *○━\|^ *[#.]=')
[ -n "$rail" ] || { echo "no rail line found in verify's output"; exit 1; }

printf '%s\n' "$rail" | grep -q 'explore propose  apply verify archive' \
    || { echo "rail label line missing or malformed"; exit 1; }

# explore not yet run - hollow. propose done (spec has criteria) - filled.
# apply: no checkbox marks anywhere, but a real run now exists - counts as
# done per stage-rail's own documented fallback. verify: the first run
# passed - filled. archive: no archived: header - hollow.
dots=$(printf '%s\n' "$rail" | head -1)
case "$dots" in
    *○*●*●*●*○*) : ;;
    *) echo "rail dots do not match expected explore=pending propose=done apply=done verify=done archive=pending: $dots"; exit 1 ;;
esac

exit 0
