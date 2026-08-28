# delta palette - the one file that knows what colour or glyph anything is.
#
# Sourced (". delta/bin/palette.sh") by delta/bin/verify (terminal) and
# delta/bin/report (HTML). Neither may write a literal colour or glyph
# character anywhere else - if a colour needs to change, it changes here
# once, and both surfaces move together. That is the whole point: the same
# run must look like the same run whether you watched it happen or opened
# the HTML report afterward.
#
# Not sourced as an executable - it only sets variables. Safe to source from
# dash, bash, or any POSIX sh.

# ---------------------------------------------------------- RGB triples ----
# Canonical form. Terminal truecolor escapes and HTML hex are both derived
# from these, not stored separately, so they cannot drift from each other.
PALETTE_RGB_BG="27 22 19"          # warm near-black ground
PALETTE_RGB_BG_PANEL="34 28 24"    # panel background, one step up (HTML only)
PALETTE_RGB_RULE="74 64 56"        # muted warm-grey rule / border
PALETTE_RGB_FG="233 225 214"       # warm off-white text
PALETTE_RGB_FG_DIM="154 143 131"   # muted warm grey - manual, pending
PALETTE_RGB_ACCENT="201 113 79"    # terracotta - the Delta glyph, headers
PALETTE_RGB_PASS="127 176 105"     # green
PALETTE_RGB_FAIL="217 99 79"       # red

# ------------------------------------------------- terminal SGR numbers ----
# The 16-colour ANSI codes are what a terminal actually renders reliably
# everywhere, including 256-colour and plain 16-colour terminals - these are
# not a reduction of the RGB triples above by any formula, they are simply
# the correct, portable choice for the same semantic colour. Only the
# gradient rule (truecolor-only, see delta/bin/verify) uses the RGB triples
# directly in the terminal; everything else uses these.
PALETTE_ANSI_PASS=32
PALETTE_ANSI_FAIL=31
PALETTE_ANSI_DIM=2
# Terracotta has no faithful 16-colour equivalent; yellow is the standard
# "warm accent" substitute and is what the completion-moment frame (see
# delta/bin/verify) falls back to on a 256- or 16-colour terminal. Only the
# gradient rule requires true 24-bit colour and simply does not render
# there - this is the one colour in the palette that degrades to a
# different hue rather than disappearing, because unlike pass/fail/dim it
# is not one of the four state colours that need to match verify.sh exactly
# across surfaces.
PALETTE_ANSI_ACCENT=33

# ----------------------------------------------------- state -> colour -----
# Documented once, applied by both surfaces:
#   passed, fixed                        -> pass (green)
#   failed, suspicious, error, no-check  -> fail (red)
#   manual-open, manual-signed, pending  -> dim (muted)
#   reproduced                           -> fg (no colour - verify's own
#                                            choice: it is neither a pass
#                                            nor a failure)

# --------------------------------------------------------------- glyphs ----
# Unicode first, ASCII fallback second - both live here so neither surface
# invents its own.
PALETTE_GLYPH_PASS_UTF8='✓';       PALETTE_GLYPH_PASS_ASCII='[ok]'
PALETTE_GLYPH_FAIL_UTF8='✗';       PALETTE_GLYPH_FAIL_ASCII='[FAIL]'
PALETTE_GLYPH_MANUAL_UTF8='○';     PALETTE_GLYPH_MANUAL_ASCII='[man]'
PALETTE_GLYPH_PENDING_UTF8='·';    PALETTE_GLYPH_PENDING_ASCII='...'
PALETTE_GLYPH_REPRO_UTF8='◆';      PALETTE_GLYPH_REPRO_ASCII='[rep]'
PALETTE_GLYPH_SUSPICIOUS_UTF8='!'; PALETTE_GLYPH_SUSPICIOUS_ASCII='[!!]'
PALETTE_GLYPH_ERROR_UTF8='⚠';      PALETTE_GLYPH_ERROR_ASCII='[err]'

PALETTE_SIGIL_UTF8='Δ';  PALETTE_SIGIL_ASCII='d'
PALETTE_RULE_UTF8='─';   PALETTE_RULE_ASCII='-'
PALETTE_SEP_UTF8='·';    PALETTE_SEP_ASCII='-'
PALETTE_ELLIPSIS_UTF8='…'; PALETTE_ELLIPSIS_ASCII='~'
PALETTE_SPIN_UTF8='⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏'
PALETTE_SPIN_ASCII='- \ | /'

# ------------------------------------------------------------ helpers -----

# palette_hex <RGB triple, e.g. "$PALETTE_RGB_ACCENT"> -> "#rrggbb"
# For the HTML surface. Pure POSIX arithmetic, no external tool.
palette_hex() {
    set -- $1
    printf '#%02x%02x%02x' "$1" "$2" "$3"
}

# palette_ansi_truecolor <RGB triple> -> the SGR escape sequence, no leading ESC
# For the terminal surface's gradient rule. Caller wraps it: printf '\033[%sm' "$(...)"
palette_ansi_truecolor() {
    set -- $1
    printf '38;2;%s;%s;%s' "$1" "$2" "$3"
}
