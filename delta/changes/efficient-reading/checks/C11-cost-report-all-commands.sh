#!/bin/sh
# CRITERION: C11 explore, propose, apply, and archive each end with what they read (file count, truth used) before the closing frame - for every shipped CLI adapter
set -eu
cd "${DELTA_ROOT:-$PWD}"

fail=0
for cmd in explore propose apply archive; do
    files="delta/commands/$cmd.md .claude/commands/delta-$cmd.md .codex/prompts/delta-$cmd.md .agents/skills/delta-$cmd.md"
    for f in $files; do
        [ -f "$f" ] || { echo "$f does not exist"; fail=1; continue; }
        grep -qi "Say what this cost" "$f" || { echo "$f has no 'Say what this cost' section"; fail=1; }
        grep -qi "truth:" "$f" || { echo "$f's cost report does not mention whether truth was used"; fail=1; }
        grep -q "^     read: " "$f" || { echo "$f has no example 'read: N files' cost line"; fail=1; }
    done
    gf=".gemini/commands/delta/$cmd.toml"
    [ -f "$gf" ] || { echo "$gf does not exist"; fail=1; continue; }
    grep -qi "Say what this cost" "$gf" || { echo "$gf has no 'Say what this cost' section"; fail=1; }
done

# The section has to sit before the closing frame, not after - a cost report
# printed after "print this last" would never actually get printed last.
for cmd in explore propose apply archive; do
    f="delta/commands/$cmd.md"
    cost_line=$(grep -n "Say what this cost" "$f" | head -1 | cut -d: -f1)
    frame_line=$(grep -n "Signature frame — print this last" "$f" | head -1 | cut -d: -f1)
    if [ -z "$cost_line" ] || [ -z "$frame_line" ] || [ "$cost_line" -ge "$frame_line" ]; then
        echo "$f: cost report does not come before the closing signature frame"
        fail=1
    fi
done

[ "$fail" -eq 0 ] && exit 0
exit 1
