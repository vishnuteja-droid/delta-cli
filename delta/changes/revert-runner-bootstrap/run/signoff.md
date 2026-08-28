# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C6 signed-off-by: build 2026-08-28 - ran delta/bin/verify against a fixture
with C1 (a valid check missing its executable bit) and C2 (a check with a
nonexistent interpreter in its shebang, exit 127 from the shell) alongside a
normal passing C3. Both C1 and C2 rendered with the [err] glyph, not [FAIL];
the summary line read "3 criteria - 1 passed - 2 error(s)" with no "failed"
in it at all; the run exited 7. Read delta/bin/verify's header: it documents
exit 7 ("at least one check could not run at all"), states the precedence
7 > 1 > 6 > 2 > 3 > 5, and the "Errors vs failures" section explains the
rationale (a check that couldn't run says nothing about whether the
criterion holds, so counting it as failed would send someone to debug
application code that is fine) and names both detection paths (the
pre-flight -x test, and exit codes 126/127 after an attempted exec). All
conditions in the criterion's confirm list are met.
