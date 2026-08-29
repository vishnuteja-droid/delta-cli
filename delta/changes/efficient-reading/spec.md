# CR-007 — Efficient reading

## Context

Delta's cost is almost entirely reading. The five command prompts decide how
the agent reads; this CR changes what they say about that, not what they
produce. Pure prompt work — no `delta/bin/*` change.

Most of these acceptance criteria describe agent behaviour, which a shell
script cannot execute the way `delta/bin/verify`'s own checks can. Where a
claim is genuinely about emergent behaviour, the criterion is MANUAL and
backed by real dogfooding (see explore.md) rather than an assertion: an
actual before/after file count in this repository's own history for
truth-first reading, and a real fixture with a deliberately wrong truth
entry and a real service boundary for stale-truth detection and bounded
traversal. Where a claim is about what the prompt *instructs*, it is
automated — checking the instruction is actually there, in every generated
CLI's copy, the same pattern already used for CR-006's rail instructions.

## MODIFIED

- `delta/commands/explore.md` — "Read first" replaced with "Read truth
  first": read `delta/truth/` before source, find what changed since the
  truth file's last commit via `git log`, investigate only the gap. Stale
  truth (code disagrees with truth) is now its own finding type. New
  "Bound the traversal" (service calls become edges, not followed; tests/
  generated/vendored code skipped unless named) and "Read cheap first"
  (grep → headers/imports → signatures → bodies, note when a body was
  needed) sections. Explicit "Stop when you have enough" section. Findings
  file is now written incrementally, not composed once at the end. New
  "Say what this cost" section before the closing frame.
- `delta/commands/propose.md` — explicit stop condition (every criterion
  measurable and checked-or-MANUAL, not "feels complete"). New "Say what
  this cost" section before the closing frame.
- `delta/commands/apply.md`, `delta/commands/archive.md` — new "Say what
  this cost" section before each closing frame.

Nothing about what any command *produces* changes: the findings file's four
headings, the spec's four sections plus acceptance criteria, the checkbox
convention, and the truth-folding rules are all unchanged.

## ADDED

None (no new files beyond this change's own directory).

## REMOVED

None.

## RENAMED

None.

## Acceptance criteria

- C1 explore's instructions say to read truth before source, check git log
      since truth's last commit for the relevant paths, and investigate
      only what truth does not answer plus what changed — for every
      shipped CLI adapter
- C2 MANUAL a second explore of an area truth already covers reads
      substantially less than the first, and says so
      reason: an emergent behaviour claim, not something a grep can prove
      look at: this change's own explore.md, "Real dogfooding" section —
        the actual first explore (13 files, CR-DOCS, before truth existed)
        versus the actual second explore (1 file, this session, against
        the real committed delta/truth/docs.md)
      confirm: the second explore's file count is substantially lower than
        the first's, using real counts from this repository's own history,
        not estimates
- C3 explore's instructions make a stale truth entry (code contradicts
      truth) its own named finding, distinct from an unknown — for every
      shipped CLI adapter
- C4 MANUAL explore actually identifies a stale truth entry when run
      against code that contradicts it
      reason: emergent behaviour, not mechanically checkable
      look at: delta/changes/efficient-reading/dogfood/ — a fixture truth
        file claiming synchronous dispatch, real fixture code that is
        fire-and-forget, and the findings file that following the new
        instructions actually produced
      confirm: the findings file names the contradiction, says code wins,
        and does not silently correct it without flagging
- C5 explore's instructions bound the traversal: a call into another
      service is recorded as an edge and not followed, and tests/generated/
      vendored code are skipped unless the intent names them — for every
      shipped CLI adapter
- C6 MANUAL explore actually treats a service boundary as an edge and
      skips a test file, on a real fixture
      reason: emergent behaviour, not mechanically checkable
      look at: the same dogfood findings file as C4 — the fixture has a
        call into an external client library (a stand-in for another
        service) and a co-located test file
      confirm: the findings file records the external call as an edge
        without describing that library's own internals, and never
        mentions the test file at all
- C7 explore's instructions state the cheap-first reading ladder in order
      (grep, headers/imports, signatures, bodies) and say to note when a
      body had to be read — for every shipped CLI adapter
- C8 explore's instructions have an explicit stop condition (entry point,
      call chain, data touched, unknowns — then stop) — for every shipped
      CLI adapter
- C9 propose's instructions have an explicit stop condition tied to
      measurability (every criterion measurable and checked-or-MANUAL, not
      a subjective "complete") — for every shipped CLI adapter
- C10 explore's instructions say to append findings to the file as each is
      established, not compose them once at the end, and say why
      (an interrupted run should leave something useful) — for every
      shipped CLI adapter
- C11 every one of explore, propose, apply, and archive ends with a line
      reporting what it read (file count, whether truth was used) before
      the closing frame, for every shipped CLI adapter — explore and
      propose additionally report on the reading ladder / bodies where
      that concept applies to them
- C12 the findings file's four required headings and the spec's four
      section headings plus Acceptance criteria are unchanged from before
      this CR — output format is identical, only how commands get there
      changed
