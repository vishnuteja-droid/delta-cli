# Findings — CR-008 a second pair of eyes

Read: delta/commands/{propose,explore,verify,archive}.md, delta/adapters.yaml,
delta/bin/generate-commands (to confirm the command list is derived, not
hardcoded), delta/changes/docs-reconciliation/checks/{C1,C3} (to find what
broke when a sixth command landed).

## Entry points

- `delta/commands/critique.md` (new) — the sixth command, generated
  automatically by the existing `delta/bin/generate-commands` glob with no
  code change.
- `delta/commands/propose.md` — new "Once approved: write, then critique"
  step.
- `delta/commands/explore.md` — new optional parallel-fan-out section.
- `delta/adapters.yaml` — new optional `roles` field per adapter.

## Data touched

- `delta/changes/<id>/critique.md` (new, per change) — findings only,
  written by critique, never read by anything that gates on it.
- `delta/adapters.yaml` — read by critique's own instructions to decide
  which isolation path to take.

## Unknowns

- Whether Antigravity CLI's and Codex's subagent mechanisms work as
  reliably in practice as their (secondary-source, egress-blocked-from-
  primary-docs) descriptions suggest. Antigravity's `roles` entry is
  declared with that caveat attached, matching the same disclosure pattern
  already used for its `user_dir` entry. Codex's is deliberately left
  undeclared after finding open upstream reports of unreliable spawn-by-name
  behaviour — declaring it would assert an isolation guarantee this table
  cannot stand behind yet.
- Whether a real end-user's subagent-spawning behaviour matches this
  session's own (Claude Code has direct, first-hand verification here: the
  dogfood run in `dogfood/` used the actual Task-tool mechanism this
  repository's own agent runs on).

## Stale truth

`delta/changes/docs-reconciliation/checks/C1-no-removed-features.sh` and
`C3-install-fresh-machine.sh` both hardcoded assumptions from before this
CR: a fixed five-command allowlist, and a literal expected file count of 20
(5 commands x 4 adapters). Adding `critique` broke both — not a stale
*truth* file (delta/truth/ has nothing about the command count), but the
same class of problem CR-DOCS's own recurring checks are meant to catch,
caught here by simply running the regression suite after adding a real
sixth command. Fixed as part of this change: C1 now names `critique` in its
known-commands list, and C3 derives its expected count from
`delta/commands/*.md` instead of a literal number, so the next command
added won't break it again the same way.
