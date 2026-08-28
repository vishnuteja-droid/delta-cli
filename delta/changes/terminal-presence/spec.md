# CR-004 — Presence, in the terminal and the UI

## Context

delta printed a frame and a list. Correct, and flat — a four-minute explore
looked identical to a stalled one. The screen should tell you what is
happening right now, and the state should change. Signal first, character
second: everything here rides on top of information worth showing anyway.

## Scoped down from the originating spec

The originating spec assumes `ui.html` exists (from a "CR-003") and asks
this change to carry the same effects into it. Neither exists in this
repository's history — there is no prior UI deliverable to extend. Rather
than invent a CR-003's worth of undocumented scope, this change is terminal-
only: `delta/bin/verify`, and the four agent-executed commands, to the
extent an agent's composed text can honestly claim to do it.

Item 7 (one shared palette, neither surface hardcoding a colour) assumed
`ui.html` as the second surface. The actual second surface that exists is
`delta/bin/report` (CR-005's HTML generator), which already had its own
independently hardcoded palette. This change gives both scripts one real
shared source — `delta/bin/palette.sh` — so the acceptance criterion about
matching colours is checkable against something real, not vacuous.

Item 8 and the two `ui.html`-specific acceptance criteria are dropped
outright, not silently: there is nothing to carry the effects into.

## What "live" means for four of the five commands

`explore`, `propose`, `apply`, and `archive` are agent-executed prompts, not
real processes. They cannot trap `SIGINT`, detect a TTY, or truly rewrite a
line in place with a running clock — none of that exists for composed
response text, and claiming it would be asserting behaviour delta cannot
verify or guarantee across every host CLI. Only `verify` is a real script
with real process and terminal control, so only `verify` gets the literal
mechanics (spinner, elapsed time, cursor handling, resize, gradient,
completion colour). The other four get the same *spirit* honestly: results
and progress printed as the work happens rather than batched at the end,
and the small honest verb vocabulary — with each command file saying
plainly that this is composed text, not a live-updating line.

## ADDED

- `delta/bin/palette.sh` — RGB triples, ANSI SGR numbers, and the glyph
  vocabulary (unicode + ASCII), in one file. `delta/bin/verify` and
  `delta/bin/report` both source it; neither defines a colour or glyph of
  its own. `palette_hex` and `palette_ansi_truecolor` convert the shared
  RGB triples for each surface's own needs (CSS hex, terminal 24-bit escape).
- A live status line in `verify`, recomputed every tick: spinner, the verb
  `running`, the criterion id and description, and an elapsed `M:SS` field -
  not just a spinner glyph.
- A gradient rule: on a truecolor terminal (`COLORTERM=truecolor|24bit`),
  the frame's `─` run fades left to right from the accent colour to the dim
  foreground, interpolated per-character from the shared RGB triples. Falls
  back to the existing flat dim rule everywhere else, silently.
- A completion moment: a clean closing frame's sigil and rule print in the
  accent colour; a failing run's summary highlights only the specific
  non-zero counts (failed, suspicious, error, unchecked) in red — never the
  whole line, and never the frame itself when failing.
- Signal handling: `INT`/`TERM`/`EXIT` traps that restore cursor visibility
  and reset colour on every path, and explicitly kill the active check
  subprocess so a check does not keep running detached after Ctrl-C.
- Resize-awareness: the live line's terminal width is recomputed from
  `/dev/tty` on every tick, not cached from script start.
- `propose` renders the spec and checks as fenced ` ```diff ` blocks, one
  per file, unified-diff form — the format every markdown-rendering
  terminal already colours correctly without delta emitting any ANSI of
  its own.
- `explore` and `apply` print transient progress lines (`reading <file>`,
  `writing <file>`) using the small honest verb vocabulary, as the work
  happens rather than batched, with each command file stating plainly that
  this is composed text, not a real live-updating line.

## MODIFIED

- `delta/bin/report`'s palette block now derives every colour from
  `delta/bin/palette.sh` via `palette_hex`, rather than its own hardcoded
  hex literals. The values are unchanged (confirmed identical byte-for-byte
  to what CR-005 shipped) - this is a refactor to a shared source, not a
  redesign.
- Every existing change's fixture `lib.sh` that copies `delta/bin/verify`
  into a scratch repo now also copies `delta/bin/palette.sh` alongside it -
  verify no longer runs without it.

## Known limitation, disclosed rather than hidden

Killing the active check's process on Ctrl-C reaches the check's own shell
reliably (confirmed: the kernel delivers the terminal's interrupt to the
whole foreground process group, which includes it). It does not reach a
grandchild process a check spawns internally (for example a check that
itself runs `sleep` or backgrounds work) - that grandchild can become an
orphan and keep running after Ctrl-C. delta cannot reach into an arbitrary
check's own process tree. Rare in practice (most checks are a single
synchronous command), but real, and worth knowing rather than silently
claiming perfect cleanup.

## Acceptance criteria

- C1 the status line updates in place and shows the current file or check, not just a spinner
- C2 results print as they complete, above the status line
- C3 MANUAL Ctrl-C at any point leaves a visible cursor and default colours
      reason: requires injecting a real terminal INTR byte into a live pty
      and inspecting the exact byte sequence that follows - reproducible,
      but the reproduction itself needs a pty-controlling tool (python's
      `pty`/`fcntl`, or `expect`) that this project does not want as a
      standing dependency of an ordinary check
      look at: run the reproduction below and read the byte stream directly
      confirm: `\x1b[?25h` (cursor show) and `\x1b[0m` (colour reset) both
      appear in the output stream after the interrupt byte, and the process
      exits via SIGINT (WIFSIGNALED, WTERMSIG=2), not by running to completion
- C4 MANUAL resizing the terminal mid-run does not corrupt the display
      reason: same constraint as C3 - needs a real pty and a live
      TIOCSWINSZ resize while the process is running, mid-check
      look at: run the reproduction below and compare the live line's
      truncation point before and after the resize
      confirm: the live line's content changes to match the new width (a
      long description truncates shorter after narrowing, not the same
      truncation point as before) and no line ever exceeds the new width
- C5 piping to a file produces clean plain text with no escape sequences and no carriage returns
- C6 NO_COLOR=1 on a TTY disables colour but keeps the frame and the results
- C7 a non-UTF-8 locale renders ASCII glyphs throughout with aligned columns
- C8 the gradient appears on a truecolor terminal and degrades silently elsewhere
- C9 propose shows the spec and checks as a coloured diff
- C10 a clean run and a failing run are distinguishable at a glance without reading the numbers
- C11 the palette and glyph vocabulary live in one file; neither the runner nor the HTML generator hardcodes a colour
- C12 the same run rendered in the terminal and in the HTML report (delta/bin/report - ui.html does not exist, see Context) uses identical state colours

## Manual reproduction for C3 and C4

```python
import os, pty, time, signal, select, struct, fcntl, termios

pid, fd = pty.fork()
if pid == 0:
    os.execvp("delta/bin/verify", ["delta/bin/verify", "<a change with a >1s check>"])
else:
    time.sleep(1.0)
    # C3: send the real terminal interrupt byte, not os.kill() on the PID -
    # os.kill() targets one process and does not reproduce what a real
    # terminal does (SIGINT to the whole foreground process group).
    os.write(fd, b'\x03')
    # C4: instead, to test resize -
    # fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 50, 0, 0))
    buf = b""
    end = time.time() + 3
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            chunk = os.read(fd, 65536)
            if not chunk: break
            buf += chunk
    print(buf)
```
