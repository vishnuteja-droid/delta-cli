#!/bin/sh
# CRITERION: C1 delta/bin/install writes only under $HOME; the repository it is run from is never touched
set -eu

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fakehome="$tmp/home"; mkdir -p "$fakehome"

# Exclude this verify run's own output - that is expected to change on every
# invocation of verify, install or not, and isn't what this criterion is about.
before=$(find . -type f -not -path './delta/changes/*/run/*' | sort)
HOME="$fakehome" delta/bin/install >/dev/null
after=$(find . -type f -not -path './delta/changes/*/run/*' | sort)

if [ "$before" != "$after" ]; then
    echo "install modified the repository it was run from:"
    printf '%s\n' "$before" > "$tmp/before.txt"
    printf '%s\n' "$after"  > "$tmp/after.txt"
    diff -u "$tmp/before.txt" "$tmp/after.txt" 2>/dev/null || true
    exit 1
fi

test -f "$fakehome/.claude/commands/delta-explore.md" || { echo "install did not write claude commands into HOME"; exit 1; }
test -f "$fakehome/.gemini/commands/delta/explore.toml" || { echo "install did not write gemini commands into HOME"; exit 1; }

# Superseded by CR-002.R: install no longer writes a runner or a constitution
# template anywhere. Confirm neither exists and nothing under .delta was
# created at all - there is no global runner, ever.
test ! -e "$fakehome/.delta" || { echo "install created \$HOME/.delta, which should no longer exist at all"; exit 1; }
