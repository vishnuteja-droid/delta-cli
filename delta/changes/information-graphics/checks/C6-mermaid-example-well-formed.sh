#!/bin/sh
# CRITERION: C6 explore's own worked Mermaid example is syntactically well-formed flowchart syntax
set -eu
cd "${DELTA_ROOT:-$PWD}"

block=$(awk '
    # A fence opens with ``` or ```<language> alike - only the closing ```
    # is ever bare, so matching just ^```$ desyncs on every language-tagged
    # block that appears earlier in the file.
    /^```/ {
        infence = !infence
        if (infence) { buf = "" } else if (buf ~ /^flowchart/) { printf "%s", buf }
        next
    }
    infence { buf = buf $0 "\n" }
' delta/commands/explore.md)

[ -n "$block" ] || { echo "no fenced 'flowchart ...' block found in delta/commands/explore.md"; exit 1; }

printf '%s\n' "$block" | head -1 | grep -qE '^flowchart (TB|TD|BT|RL|LR)$' \
    || { echo "first line is not a valid 'flowchart <direction>' header"; exit 1; }

body_tmp=$(mktemp); trap 'rm -f "$body_tmp"' EXIT
printf '%s\n' "$block" | tail -n +2 > "$body_tmp"
[ -s "$body_tmp" ] || { echo "flowchart has a header but no nodes or edges"; exit 1; }

# Every non-blank body line is an edge (--> or -.->), and every open
# bracket/paren has its matching close on the same line, since Mermaid
# nodes are declared inline.
while IFS= read -r line; do
    [ -n "$(printf '%s' "$line" | tr -d '[:space:]')" ] || continue
    case "$line" in
        *'-->'*|*'-.->'*) : ;;
        *) echo "line has no edge arrow: $line"; exit 1 ;;
    esac
    opens=$(printf '%s' "$line" | tr -cd '[(' | wc -c)
    closes=$(printf '%s' "$line" | tr -cd '])' | wc -c)
    [ "$opens" -eq "$closes" ] || { echo "unbalanced brackets on line: $line"; exit 1; }
done < "$body_tmp"

exit 0
