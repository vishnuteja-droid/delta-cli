# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C6 signed-off-by: build 2026-08-26 - read the "Bug fixes: when the cause is
unknown" section in delta/commands/propose.md and the same text in the
generated .claude/commands/delta-propose.md. It requires ## Observed and
## Expected sections before anything else; it states "The first criterion is
the reproduction. Before any criterion about the fix..." with the
EXPECT: fail-until-fixed header shown; and it permits an empty MODIFIED
explicitly ("Leave MODIFIED empty if you do not know") while forbidding an
invented one. It also covers the suspicious case. All three conditions met.
