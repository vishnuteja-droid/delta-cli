# Fixture helpers for the CR-008 checks. No C<n> prefix, so never bound to a
# criterion.

VERIFY_SRC=${VERIFY_SRC:-$PWD/delta/bin/verify}
PALETTE_SRC=${PALETTE_SRC:-$PWD/delta/bin/palette.sh}
STAGE_RAIL_SRC=${STAGE_RAIL_SRC:-$PWD/delta/bin/stage-rail}
CRITIQUE_SRC=${CRITIQUE_SRC:-$PWD/delta/commands/critique.md}

# mk_repo <dir>
mk_repo() {
    d=$1
    mkdir -p "$d/.git" "$d/delta/bin" "$d/delta/changes" "$d/delta/commands"
    cp "$VERIFY_SRC" "$d/delta/bin/verify"; chmod +x "$d/delta/bin/verify"
    cp "$PALETTE_SRC" "$d/delta/bin/palette.sh"
    [ -f "$STAGE_RAIL_SRC" ] && { cp "$STAGE_RAIL_SRC" "$d/delta/bin/stage-rail"; chmod +x "$d/delta/bin/stage-rail"; }
    cp "$CRITIQUE_SRC" "$d/delta/commands/critique.md"
}

# mk_change_with_spec <dir> <change-id> <spec-body>
mk_change_with_spec() {
    d=$1/delta/changes/$2
    mkdir -p "$d/checks"
    printf '%s\n' "$3" > "$d/spec.md"
}
