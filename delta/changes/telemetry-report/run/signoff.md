# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C8 signed-off-by: build 2026-08-28 - CR-004 does not exist in this
repository's history, so there is no shared palette to diff against; this
is disclosed in the spec and in delta/bin/report's own header comment
rather than invented. What was actually checked: delta/bin/verify's own
glyph-to-colour assignments (grep "emit \"\$G_" delta/bin/verify) show
manual-open/manual-signed -> C_DIM, no-check/error/suspicious/failed ->
C_FAIL, reproduced -> "" (no colour, printed in the terminal's default
foreground), fixed/passed -> C_PASS. Cross-checking report's CSS and class
usage against that exact mapping surfaced two real mismatches, both fixed
before this sign-off: Q4's still-open reproduction row was rendering in the
fail/red family, when verify itself prints an open reproduction with no
colour at all (now class="neutral", plain foreground); and the MANUAL
swatch in Q2's legend and Q3's chart was using the terracotta accent color,
when verify treats manual as dim/muted, not accented (now uses --fg-dim,
matching C_DIM). Also fixed in passing: the error count's swatch in Q2's
legend was reusing Q3's unrelated "auto" class name, which happened to
render a colour but under the wrong semantic label - gave it its own
.sw.error class, coloured to match C_FAIL (the same red as an ordinary
failure), since that is what verify itself does - it does not distinguish
error from failure by colour either, only by count and label, and neither
does the report now. After these fixes: passed/fixed = green (--pass),
failed/suspicious/error = red (--fail), manual/pending = dim (--fg-dim),
reproduced = plain foreground (--fg, via .neutral) - matching verify.sh's
actual C_PASS/C_FAIL/C_DIM/no-colour scheme exactly, criterion by
criterion, not just by eye.
