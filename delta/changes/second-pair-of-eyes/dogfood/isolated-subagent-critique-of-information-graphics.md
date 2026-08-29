# Real dogfood run: isolated subagent, critique of information-graphics/spec.md

Spawned via Claude Code's actual Task-tool subagent mechanism (the same
mechanism `delta/adapters.yaml`'s `roles` entry for `claude` describes) —
genuinely isolated, zero inherited context from this conversation. Given
only the instruction to read `delta/changes/information-graphics/spec.md`
and `delta/constitution.md`, apply critique's six categories, and report
back with confirmation of which files it read. Not asked to be lenient or
harsh — asked to do the job.

Full agent output verbatim below.

---

## Findings

**C6 / C5 — category 5 (conflict) and category 2 (shallow check).** C5's MANUAL confirm-clause says the Mermaid block using the same node names as the diagram is "checked structurally, not rendered, by C6." But C6's own definition text only requires the Mermaid example be "syntactically well-formed flowchart syntax (a valid header, every body line an edge, balanced brackets)" — nothing in C6 as written cross-checks node names against the ASCII diagram. As stated, C6 would pass on a Mermaid export whose node names diverge entirely from the box-drawing diagram, silently breaking the very guarantee C5 leans on to justify not re-checking it manually.

**C1 — category 2.** "Shows the correct stage for each of the five positions" doesn't say what the check validates "correct" against. The Context section describes the exact failure mode this change was written to fix: the rail reading stale/in-progress run state as if it were a completed one. A check that only confirms the rail prints plausible-looking stage labels — without diffing against actual on-disk run state for each of the five stages — could pass while a same-shape staleness bug persists or reappears elsewhere in the rail's logic.

**C2 — category 1 and category 4.** "Stays red on subsequent commands until resolved" never defines "resolved" — a later passing run of that specific stage? any later run? a manual override? That ambiguity makes the criterion hard to check objectively as written. It also silently assumes some persistent, cross-command run-history source that stage-rail consults to decide "still failed" vs "resolved" — but the ADDED bullet for `stage-rail` only says it "computes and prints," never naming a history source the way the `verify --all` bullet explicitly names `run/*/results.tsv`. The spec never states stage-rail depends on that same file, yet C2 requires exactly the kind of persistence only that file could supply.

**verify --all — category 3 (missing failure mode).** The ADDED bullet states `verify --all` is read-only and "never executes a check" — a safety property of the dashboard. No acceptance criterion verifies this. Nothing checks that invoking `--all` doesn't itself trigger a check run, which would be the actual dangerous failure mode for a feature explicitly billed as read-only.

**C8 — category 3 (missing coverage).** The ADDED text promises the heatmap gives "passed/failed/could-not-run/manual each a distinct palette colour" — four states. C8 only requires three distinct colours (failed, could-not-run, manual) and says nothing about "passed." A heatmap where "passed" collides with one of the other three colors would still satisfy C8 while breaking the feature as described.

**Report's heatmap — category 3 (missing failure mode).** C10 requires `verify`, `verify --all`, and `stage-rail` to emit zero escape sequences when not on a TTY. `delta/bin/report`, which now carries the new colored heatmap (C8), is absent from C10 and has no equivalent criterion of its own — despite the CR stating up front that it inherits CR-004's "no TTY means plain output, NO_COLOR respected" constraints across the board.

**C5 — category 6 (MANUAL that could be automated).** C5 bundles three confirm-clauses under one MANUAL label. Two are genuinely unautomatable ("reads clearly," renders correctly in a GitLab this environment doesn't have). But "the unknown sits under the node it questions rather than in a trailing list" is a structural/positional property of the diagram text — parseable mechanically (is the unknown-marker line adjacent to its node vs. grouped at the end) rather than folded into the same manual-judgment bucket.

**C3 / C4 — category 2 (minor, shallow check).** "All instruct running `delta/bin/stage-rail` literally" and the explore-instruction requirements are phrased in a way a naive keyword/substring check could satisfy — e.g., a file that merely mentions "stage-rail" in an unrelated or negated sentence would match a substring check without actually instructing the reader to run it.

## Confirmation

Read exactly two files, both under `/home/user/delta-cli`:
1. `delta/changes/information-graphics/spec.md`
2. `delta/constitution.md` — note: this file is the unfilled template (every section still reads `...`/example comments), so there was no actual constitution content to check criteria against; category 5's constitution-conflict check found nothing to compare against, not because nothing was searched.

No other file was read (no `explore.md`, no source code, no other change's spec).

---

## What this demonstrates

- **Genuine isolation, not asked-for restraint.** The subagent had zero
  access to this conversation's history — it could not have "already known"
  information-graphics' own actual checks/*.sh (which do, in fact, verify
  most of these gaps mechanically even though spec.md's criterion *text*
  doesn't say so explicitly). It found gaps in the spec's own wording, which
  is exactly the restricted-context job.
- **Honest about a limitation it hit.** It did not fabricate a
  constitution-conflict finding to look thorough when the constitution
  turned out to be the unfilled template - it said so plainly (matching the
  "reports finding nothing rather than inventing objections" requirement,
  extended honestly to "found nothing to check against").
- **Real category 6 catch.** Flagging that C5's "unknown sits under the
  node it questions" sub-clause is a structural check bundled into a MANUAL
  is exactly the kind of finding CR-008's own Notes section says directly
  improves CR-005's testability metric.
- **Findings only.** The subagent was never given tool access beyond
  reading the two named files - it could not have edited spec.md even if
  its instructions had asked it to (they didn't).
