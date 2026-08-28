# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C3 signed-off-by: build 2026-08-28 - ran the spec's reproduction (a real pty
via pty.fork(), a >1s check, os.write(fd, b'\x03') at t=1s - the actual
terminal INTR byte, not os.kill() on a single PID, since that does not
reproduce what a real terminal's Ctrl-C does: SIGINT to the whole foreground
process group). Confirmed \x1b[?25h (cursor show) and \x1b[0m (colour reset)
both appear in the byte stream immediately after the ^C, and the process
exited via WIFSIGNALED with WTERMSIG=2 (SIGINT) rather than running the
check to completion. Also separately confirmed (documented in this change's
spec, "Known limitation" section) that a check's own grandchild process -
one it spawns internally, e.g. a check that itself calls sleep - can survive
as an orphan after Ctrl-C, since the kernel's process-group signal only
reaches the check's own shell, not arbitrary descendants a check spawns
itself. That is disclosed as a known limitation, not silently claimed fixed.

C4 signed-off-by: build 2026-08-28 - ran the spec's reproduction (real pty,
a fixture criterion with a genuinely long description, TIOCSWINSZ resize
from 100 to 50 columns one second into a run). The live line's truncation
point visibly changed - "...will need real trunca…" at 100 columns versus
"...a criterion the genuinel…" at 50 - and the post-resize line stayed
within the new 50-column width. Also separately confirmed at rest (not part
of this specific reproduction, but the same mechanism): a plain
`COLUMNS=N delta/bin/verify` sweep at 46/60/80/100 columns produces zero
lines exceeding the given width.
