---
description: Read the code behind an area and write down what is actually there, including what could not be determined.
argument-hint: "<area>"
---

## Signature frame — print this first

The very first line of your response is the opening frame. Nothing precedes it.

      Δ ─────────────────────────────────  delta explore

Exact characters: `Δ` (U+0394), `─` (U+2500), `·` (U+00B7). Two leading spaces,
`Δ`, one space, the rule, two spaces, the label. Pad the rule so the line is
about 60 characters wide. If the terminal is not UTF-8, substitute `d` for `Δ`,
`-` for `─`, and `-` for `·`.

The very last line is the closing frame, described at the bottom of this file.
A closing frame is how the reader knows you ran this command to completion
instead of drifting off part-way, so it is not optional and not decorative.

## Where this repository's delta/ lives, if it has one

Resolve the root the way git resolves its own, before doing anything else:

1. If `$DELTA_ROOT` is set, use it without searching.
2. Otherwise check the current directory, then each parent in turn, for a
   `delta/` directory. The first one found is the root, and it already has a
   `delta/` you can write into.
3. If none is found, do the same walk again looking for `.git` instead. That
   is the root of the repository — but it has no `delta/` yet, meaning this
   repository has never run `propose`. **Do not create `delta/` here.**
   Explore never creates it; only `propose` does, and only when it needs to.
   Continue anyway, writing findings to the terminal only (see below).
4. If neither is found, this is not inside a repository. Say so and stop.

If the resolved root is not the current directory, say so before doing
anything else: `root: /path/to/repo`. Every `delta/...` path below is
relative to that root, not to the current directory.

## What you are doing

Area: $ARGUMENTS

You are building understanding *before* anyone proposes a change to it.

If `delta/` exists at the resolved root, write the output as a findings file
at `delta/changes/<id>/explore.md`, where `<id>` is a short kebab-case slug
for the area — creating `delta/changes/<id>/` if it does not exist yet, but
never creating `delta/` itself. If `delta/` does not exist at all, explore has
nothing to write into: print the same findings to the terminal in full and
stop there. Do not create `delta/` to make a place to put them — that is
`propose`'s decision to make, not this command's.

This is the step that makes the following `propose` worth trusting. A findings
file that restates what the code obviously says has failed, even if everything
in it is true.

## Read first

Read `delta/constitution.md` if it exists at the resolved root. Read
`delta/truth/` if it exists and is not empty — that is what is already
understood, and you are looking for what it does not yet cover. Neither
existing is a normal, common state for a repository that has never run
`propose`; proceed without them.

## Find these four things

1. **Entry points.** Where does control actually enter this area? Route
   handlers, message consumers, scheduled jobs, CLI subcommands. Name the
   symbol and the file.
2. **Call chain.** What calls what, in order, down to the thing that touches
   state. Compress it to one line per hop.
3. **Data touched.** Tables, collections, queues, caches, external services.
   For each, whether this area reads it, writes it, or both.
4. **Unknowns.** What you could not determine from the code, stated plainly.

## Unknowns are the point

The fourth section is the one with real value, and the one a model is most
tempted to skip. Write down anything you could not establish by reading:
behaviour that depends on configuration you cannot see, a value that arrives
from another system, a code path with no visible caller, a comment that
contradicts the code, retry or timeout semantics buried in a framework.

"I could not determine X" is a finding. Guessing at X and writing it as fact
is the specific failure this whole lifecycle exists to prevent — the next
command builds a spec on top of whatever you write here.

Never pad the unknowns section to look thorough, either. If you genuinely
established everything, say so and move on.

## Empty or absent areas

If the area does not exist in this repository, or exists but is empty, write
the findings file with the four headings present and empty, state in one line
that the area is absent or empty, and stop. Do not stall, do not ask a
clarifying question first, and do not invent a plausible structure for code
that is not there. An empty findings file is a correct answer.

## Shape of the output

Short. A findings file is notes, not a document — if it reads like
documentation you have written the wrong thing. Prefer a table or a list of
one-line entries over prose. No summary section, no restatement of the four
headings at the end, no recommendations: recommending a change is the next
command's job and doing it here contaminates it.

## Print to the terminal

If a findings file was written, print the condensed form between the frames —
not the whole file:

     entry    PaymentController.dispatch
     chain    eapi → papi → sapi → ledger
     unknown  retry count comes from config

Three-space indent, label column padded to 8, then the value. One line per
item. If there are more than about eight lines, print the entry points and the
unknowns and say how many other findings are in the file.

If no `delta/` exists and nothing was written, print the full findings in that
same format instead of a condensed version of them — the terminal is the only
copy, so print all of it, not a summary of it.

## Signature frame — print this last

The final line of your response, after everything else:

      Δ ──────────────  4 findings · 2 unknowns · 3.2s

Same characters and padding rules as the opening frame. The counts must be the
real ones from the file you just wrote. Never write "done", never write a
placeholder count, and never invent an elapsed time — if you did not measure
one, leave the time off and close with the counts alone.
