#!/bin/sh
# CRITERION: C4 commands still work in a repo that has never seen delta
set -eu

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fakehome="$tmp/home"; mkdir -p "$fakehome"
HOME="$fakehome" delta/bin/install >/dev/null

# A repo with no delta/ at all - install must have made the commands
# reachable regardless, per the machine-level command files it wrote.
test -f "$fakehome/.claude/commands/delta-explore.md" || {
    echo "install did not make /delta-explore available"; exit 1; }
test -f "$fakehome/.gemini/commands/delta/explore.toml" || {
    echo "install did not make /delta:explore available"; exit 1; }

# The command's own root-walk must correctly report "nothing here yet" for
# a brand-new repo, rather than erroring or assuming the wrong root.
newrepo="$tmp/newrepo"; mkdir -p "$newrepo/.git" "$newrepo/src"
found=$(cd "$newrepo/src" && sh -c '
    d=$PWD
    while [ -n "$d" ]; do
        [ -e "$d/delta" ] && { echo "$d"; exit 0; }
        [ "$d" = "/" ] && exit 1
        d=$(dirname -- "$d")
    done
    exit 1
' || true)
test -z "$found" || { echo "a delta/ was found in a repo that never ran propose: $found"; exit 1; }
