# CR-008 — A second pair of eyes

## Context

`propose` reviews its own spec — the context that decided what to build
then judges whether the decision is sound, which catches sloppiness but
never blind spots. This adds `critique`: a sixth command, deliberately
restricted to reading only the spec and the constitution, so it has to ask
whether the spec stands on its own rather than reconstructing and agreeing
with the author's reasoning.

Most of what makes the restriction real is prompt discipline, not code —
`delta/bin/generate-commands` already globs `delta/commands/*.md`, so a
sixth command file is picked up with no script change. The one piece of
real infrastructure is `adapters.yaml`'s new `roles` field: where a tool can
spawn an isolated sub-context (a real subagent, not just an instruction to
ignore what's already been read), critique and explore's optional parallel
fan-out use it; where it can't, both fall back to a fully-specified
sequential path that is never a second-class citizen.

Adding a sixth command exposed two checks in `docs-reconciliation` that had
hardcoded "five" as a magic number rather than deriving it — fixed in this
change (see explore.md's "Stale truth" section) since they'd otherwise be
red the moment this change lands.

## ADDED

- `delta/commands/critique.md` — reads exactly `delta/changes/<id>/spec.md`
  and `delta/constitution.md`. Explicitly excludes `explore.md`, the
  original intent, and the code. Looks for unmeasurable criteria, a check
  that would pass while the feature is broken, missing failure modes,
  unstated assumptions, conflicts, and MANUAL criteria that could be
  automated. Writes findings to `delta/changes/<id>/critique.md`, never
  edits. Says `No findings.` plainly when there is nothing to report. Skips
  its own signature frames when invoked automatically by `propose` (that
  response's own frame is the true last line); prints them when invoked
  standalone.
- `roles` field in `delta/adapters.yaml` — optional, documents how a tool
  spawns an isolated sub-context. Declared for `claude` (the Task tool,
  directly verified — this is the mechanism this repository's own agent
  runs on) and, with a secondary-source caveat matching the existing
  `user_dir_verified` precedent, for `antigravity`. Deliberately not
  declared for `gemini` (the CR's own stated fact: no such mechanism) or
  `codex` (mechanism exists per research, but current reports describe
  spawn-by-name as unreliable — declaring it would assert a guarantee this
  table cannot stand behind).
- "Once approved: write, then critique" step in `delta/commands/propose.md`
  — after the developer approves and the files are written, run critique
  and present its findings before propose's own closing frame.
- "Parallel exploration, where roles exist" section in
  `delta/commands/explore.md` — optional fan-out across independent call
  chains when `roles` is declared, one sub-context per chain, findings
  merged. Sequential remains the reference implementation and always
  correct; a parallel run must produce the same findings a sequential one
  would.

## MODIFIED

- `delta/changes/docs-reconciliation/checks/C1-no-removed-features.sh` —
  known-commands allowlist now includes `critique`.
- `delta/changes/docs-reconciliation/checks/C3-install-fresh-machine.sh` —
  expected file list and count now derived from `delta/commands/*.md`
  instead of a hardcoded list of 20 files, so a future command addition
  doesn't break this check again the same way.
- `README.md`, `AGENTS.md` — six commands instead of five; a new "A second
  pair of eyes" section in the README.

## REMOVED

None.

## RENAMED

None.

## Acceptance criteria

- C1 critique's instructions read the spec and constitution only, and
      explicitly exclude the exploration findings and the code — for every
      shipped CLI adapter
- C2 critique runs at the end of `propose` and is independently re-runnable
      (defaults to the most recently modified change like every other
      re-runnable command) — for every shipped CLI adapter
- C3 critique's instructions say output is findings only, never edits to
      spec.md or checks/, and name the findings file — for every shipped
      CLI adapter
- C4 critique's instructions require reporting `No findings.` rather than
      manufacturing an objection, and say so explicitly — for every shipped
      CLI adapter
- C5 critique's instructions fully specify both the isolated-subagent path
      (when `roles` is declared) and the sequential path (when it is not),
      and `adapters.yaml` actually declares `roles` for at least one
      adapter — for every shipped CLI adapter
- C6 nothing in `delta/bin/verify` or `delta/commands/archive.md` gates on
      critique or its findings; critique's findings are written to the
      change folder, not just printed
- C7 explore's parallel fan-out is described as optional in every case, and
      required to produce output equivalent to the sequential path — for
      every shipped CLI adapter
- C8 `delta/adapters.yaml` does not declare `roles` for Gemini CLI, and
      critique's sequential (no-roles) path is fully specified rather than
      a degraded stub
- C9 MANUAL a real isolated-subagent critique run, given only a real spec
      and the real constitution, produces genuine, non-fabricated findings
      and is honest about a limitation it hits
      reason: emergent behaviour under genuine isolation is not something a
        grep can confirm — the point is whether the restriction actually
        produces a different, useful result, not whether the prompt asks
        for one
      look at: delta/changes/second-pair-of-eyes/dogfood/ — a real
        Task-tool subagent, spawned with zero access to this conversation,
        given only spec.md and constitution.md for `information-graphics`
      confirm: the findings are specific to that spec's actual text (not
        generic advice), the subagent correctly reports that
        constitution.md was still the unfilled template rather than
        fabricating a constitution-conflict finding, and spec.md /
        constitution.md are byte-identical before and after the run
