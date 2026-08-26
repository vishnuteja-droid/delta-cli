---
description: Write a delta spec against current truth, with an executable check for every acceptance criterion.
argument-hint: "<intent>"
---

## Signature frame — print this first

The very first line of your response is the opening frame. Nothing precedes it.

      Δ ─────────────────────────────────  delta propose

Exact characters: `Δ` (U+0394), `─` (U+2500), `·` (U+00B7). Two leading spaces,
`Δ`, one space, the rule, two spaces, the label. Pad the rule so the line is
about 60 characters wide. If the terminal is not UTF-8, substitute `d` for `Δ`,
`-` for `─`, and `-` for `·`.

The very last line is the closing frame, described at the bottom of this file.
A closing frame is how the reader knows you ran this command to completion
instead of drifting off part-way, so it is not optional and not decorative.

## What you are doing

Intent: $ARGUMENTS

Write `delta/changes/<id>/spec.md` — a spec describing only what *changes*,
diffed against what is currently true. Then write one executable check per
acceptance criterion into `delta/changes/<id>/checks/`.

Use the existing change directory if `explore` already made one for this area.

## Read first, in this order

1. `delta/constitution.md` — inherited by every change. Its rules constrain
   what you are allowed to propose. A spec that violates it is invalid, not a
   judgement call.
2. `delta/truth/` — the current understanding of the system.
3. `delta/changes/<id>/explore.md` — findings, if explore has run.

The unknowns section of the findings file is the most important thing you will
read. Anything listed there is something you must not silently assume.

## A delta, not a description

The spec has four sections. Every item goes in exactly one:

- **ADDED** — did not exist before
- **MODIFIED** — exists and changes behaviour; state the before and the after
- **REMOVED** — exists and goes away
- **RENAMED** — same thing, different name; state both names

This is what keeps specs small on a mature codebase forever. You are not
describing the system, you are describing the difference. If an item does not
change, it does not belong in the spec no matter how relevant it seems.

Target around 250 lines. If you are approaching 800 you have started describing
the system instead of the change, and the fix is to delete, not to condense.

## Greenfield is not a special mode

If `delta/truth/` is empty, nothing exists yet, so every item is ADDED and the
other three sections say "None." That falls out of the format; it is not a
different command.

What is different is that there is no existing system to infer intent from, so
ask the scoping questions that the code would otherwise answer:

- what is in scope
- what is explicitly out of scope
- who uses this and how they reach it

Ask these before writing, not after.

## Acceptance criteria

Write them under a `## Acceptance criteria` heading, one per line, numbered
`C1`, `C2`, ... An id is an uppercase prefix and a number, so a bug spec can
number its reproductions `B1`, `B2`. The runner parses this section, so the
format is load-bearing:

    ## Acceptance criteria

    - C1 retry returns 200
    - C2 existing ledger row is untouched
    - C3 duplicate webhook creates no second ledger row
    - C4 MANUAL error messages are readable by support staff
          reason: judgement about wording, with no assertable output
          look at: the three error strings in WebhookController
          confirm: a support reader can act on each without reading code

Each criterion states an observable outcome. "The code handles retries" is not
a criterion. "A second webhook with the same provider_ref creates no second
ledger row" is.

## Then write a check for each one

This is the only command in the lifecycle where a model touches verification,
and it happens once, in the open, under review. Take it seriously.

A check is **an executable file** — not a description, not a prompt, not a
YAML entry. A file with a shebang that exits 0 when the criterion holds and
non-zero when it does not:

    #!/bin/sh
    # CRITERION: C3 duplicate webhook does not create a second ledger row
    curl -sf -X POST localhost:8081/webhook/retry -d @fixtures/dup.json > /dev/null
    test "$(psql -tAc "select count(*) from ledger_entry where provider_ref='X1'")" = "1"

Rules that make the binding work:

- Name it `C<n>-<short-slug>.sh`. The runner binds a check to a criterion by
  that `C<n>` filename prefix, so `C3-` serves `C3`.
- First line a shebang; second line `# CRITERION: C<n> <the criterion text>`.
- Make it executable. A check without the executable bit is reported as a
  failure, which is correct but wastes a run.
- It runs with the working directory at the repository root. `$DELTA_CHANGE_DIR`,
  `$DELTA_CHECK_DIR`, and `$DELTA_RUN_DIR` are exported if it needs its own
  fixtures.
- Call whatever is already on the machine — curl, psql, mvn, npm, jq, `gh`.
  Nothing is installed to run a check.

## What a check is for

Not to catch what the test suite misses. That is the test suite's job and it
is better at it.

A check binds a stated criterion to a proof that ran. A check whose entire body
is `mvn test -Dtest=WebhookTest#idempotent` is a **good check** — the value is
the mapping and the fact that nobody can declare this change done without it.
Expect most checks to be thin wrappers over tests that already exist, and write
them that way without apology. Reaching for something more elaborate usually
means you are re-testing rather than binding.

## MANUAL criteria

Some criteria are real and cannot be automated: "the error messages are clear
to support staff", "the retry does not hammer the downstream". Mark them
`MANUAL` with a stated reason, what a human should look at, and what confirms
it.

Two things you must not do:

- **Never auto-pass one.** The runner exits 3 until a human records a sign-off.
- **Never write a weak proxy check** so the criterion technically executes.
  A check that asserts an error string is non-empty, standing in for "the
  message is clear", is worse than an honest MANUAL: it converts an open
  question into a false green.

How many criteria land as MANUAL is itself information about how testable the
spec was. Do not tune the number in either direction.

## Bug fixes: when the cause is unknown

Sometimes the intent is a symptom — "duplicate webhooks create two ledger rows",
"the export times out for large accounts" — not a change you can describe. A
bug fix does not begin with knowing what to build, and forcing one into the
normal shape produces a guess dressed as a spec.

When the intent describes something going wrong rather than something to build:

**Open the spec with the behaviour, not the change.** Two short sections before
anything else:

    ## Observed
    A webhook redelivered with the same provider_ref inserts a second
    ledger_entry row.

    ## Expected
    The second delivery is accepted and inserts nothing.

**The first criterion is the reproduction.** Before any criterion about the fix,
write a criterion that states the bug, and give its check the header
`# EXPECT: fail-until-fixed`:

    - B1 a duplicate webhook creates a second ledger row

    #!/bin/sh
    # CRITERION: B1 a duplicate webhook creates a second ledger row
    # EXPECT: fail-until-fixed
    ...

`verify` reports that as `reproduced` rather than failed, and the run exits 0 —
the bug is confirmed and the fix is not written yet. When the fix lands and the
check passes, `verify` flips it permanently into an ordinary regression guard
and records the timestamps. That record is the evidence the fix worked.

If the reproduction passes the first time you run it, the criterion is wrong —
you have not captured the bug. `verify` reports that as `suspicious` and exits
6. Fix the reproduction; never write around it.

**Leave MODIFIED empty if you do not know.** For a bug, the delta spec is a
hypothesis that firms up during `apply`, not a plan written upfront. Write
`MODIFIED: to be determined during apply — cause not yet known` and move on.

Do not guess at which files change so the section looks complete. An invented
MODIFIED entry is worse than an empty one: it sends `apply` to the wrong place
and it reads as knowledge you do not have.

Everything else is unchanged — same five commands, same spec format. A bug is
not a different lifecycle.

## Interrogate your own output before finishing

Re-read what you have written and hunt for the three things that cause a spec
to fail during `apply`:

- **Vague adjectives** — fast, robust, clean, efficient, user-friendly. Each is
  an unanswered question wearing a confident coat.
- **Unquantified thresholds** — "retries a few times", "times out quickly",
  "handles large payloads". A number, or it is not a requirement.
- **Undefined error behaviour** — for every path you added, what happens when
  it fails? Which error surfaces, to whom, and what does the caller see?

Then ask the user about everything on that list you could not answer from the
code itself. Ask the questions plainly and specifically, together, at the end.
Do not guess and do not paper over a gap with a plausible default.

## Self-check the requirements, and only the requirements

Before presenting, check the spec for:

- criteria that are not measurable
- edge cases with no criterion covering them
- criteria that contradict each other or the constitution
- items in the wrong one of the four sections

**Check the spec. Never check the implementation.** Do not evaluate whether the
change is a good idea, whether the design is sound, whether there is a simpler
approach, or how you would build it. That line is hard and it exists for a
reason: a model that reviews its own future implementation here will write a
spec shaped to be easy to satisfy rather than one that says what is needed.

## Nothing is written without approval

Present the spec and every check as a diff, and wait.

The checks especially: they are the standard the work will be measured against,
and they were written by a model. A reviewer must see each one before it lands.
Write nothing to disk until the user approves. If they change a criterion,
change its check with it.

## Print to the terminal

The spec sections with counts, the criteria list, and the questions you need
answered. Then the diff.

## Signature frame — print this last

The final line of your response, after everything else:

      Δ ──────────────  6 criteria · 5 checks · 1 manual · 3 questions

Same characters and padding rules as the opening frame. The counts must be real
and must add up: criteria, checks written, MANUAL criteria, and open questions.
Never write "done" and never invent an elapsed time.
