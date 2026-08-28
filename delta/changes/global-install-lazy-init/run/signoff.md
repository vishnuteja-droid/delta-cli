# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C8 signed-off-by: build 2026-08-28 - read the "## Where this repository's
delta/ lives" section in the generated explore, propose, apply, and archive
files (.claude/commands/delta-*.md). All four name $DELTA_ROOT as the
override checked first; all four walk the current directory then each parent
for delta/; explore and propose additionally fall back to walking for .git
when no delta/ exists, and both state clearly what to do in that case
(explore: never create delta/, print findings to the terminal only; propose:
proceed to lazy-init); apply and archive instead stop with a message pointing
at /delta:propose when no delta/ is found, which is correct since both
require an existing change. All four announce `root: /path/to/repo` when the
resolved root differs from cwd. Read propose's "## Creating delta/ for the
first time" section: it writes the constitution template verbatim with an
explicit instruction not to fill in blanks, invent rules, or introspect the
repository, bootstraps the runner via a plain file copy from
~/.delta/bin/verify with chmod +x, and explicitly refuses to fabricate a
runner by hand if that copy is missing, exiting instead. All conditions in
the criterion's confirm list are met.

Noted, non-blocking: the embedded constitution's code fence is not indented
to match its parent numbered-list item, so it is not strict CommonMark list
nesting. Since the consumer here is an LLM agent reading fence delimiters,
not a strict renderer, this does not affect comprehension of the instruction.
Worth a cosmetic pass later; not a correctness issue for this criterion.
