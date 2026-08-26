# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C6 signed-off-by: build 2026-08-26 - ran the command above at COLUMNS=46 with
LANG=C, piped through cat. Frame stayed on one line, descriptions truncated
with a trailing marker instead of wrapping, the [ok]/[man] and duration columns
stayed aligned, no line exceeded 46 characters, and the closing line read
"6 criteria - 5 passed - 1 manual".
