#!/bin/sh
# CRITERION: C5 project-level and user-level command output always land at different absolute paths
set -eu

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
HOME="$tmp" delta/bin/install >/dev/null

check_pair() {  # check_pair <project-relative-path> <user-relative-path-under-tmp>
    projabs=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
    test -f "$1" || { echo "missing project-level file: $1"; exit 1; }
    test -f "$2" || { echo "install did not write $2"; exit 1; }
    test "$projabs" != "$2" || { echo "project and user paths collided: $projabs"; exit 1; }
}

check_pair .claude/commands/delta-explore.md            "$tmp/.claude/commands/delta-explore.md"
check_pair .gemini/commands/delta/explore.toml           "$tmp/.gemini/commands/delta/explore.toml"
check_pair .agents/skills/delta-explore.md                "$tmp/.gemini/config/skills/delta-explore.md"
check_pair .codex/prompts/delta-explore.md                "$tmp/.codex/prompts/delta-explore.md"
