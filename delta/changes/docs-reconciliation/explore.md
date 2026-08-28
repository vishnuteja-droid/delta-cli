# Findings — docs vs shipped code

Read: README.md, AGENTS.md, delta/constitution.md, delta/commands/*.md,
header comments in delta/bin/*, delta/adapters.yaml, CHANGELOG.md (absent).
Compared against: delta/bin/verify's actual exit paths, delta/bin/install's
actual output, delta/bin/generate-commands --check, a real `find` of
delta/, .claude/commands/, .gemini/commands/, .agents/skills/,
.codex/prompts/.

## Drift found

- `delta/commands/verify.md` (and its four generated CLI files) said "exits
  with a code from 0 to 6" and its exit-code list stopped at 6. `delta/bin/verify`
  has had exit 7 (`error`, added by CR-002.R) since before this repo's most
  recent shipped change. `bb4c419` had already fixed this file once, for
  CR-001's codes 5/6 — CR-002.R's exit 7 landed after and was never
  propagated here. Reproduced: grepped `exit [0-9]` in delta/bin/verify (7
  present) against the code list in verify.md (stopped at 6).
- `AGENTS.md`'s exit-code summary listed 0, 1, 2, 3, 6, 7 but never 4.
- `delta/commands/propose.md`, `delta/commands/verify.md`, README, and
  AGENTS.md all instructed copying only `delta/bin/verify` into a repo that
  doesn't have delta yet. Since CR-004, `delta/bin/verify` sources
  `delta/bin/palette.sh` unconditionally (`. "$script_dir/palette.sh"`, no
  fallback). Reproduced: copied only `delta/bin/verify` into a scratch repo
  and ran it — `delta/bin/verify: 78: .: cannot open .../palette.sh: No such
  file`, exit 2. That exit code collides with delta's own documented meaning
  of exit 2 ("criterion has no check"), which is actively misleading. Fixed
  the docs to say "copy both files"; the raw-error/colliding-exit-code
  behaviour itself is a code defect, not a docs one — flagged in
  CHANGELOG.md's reconciliation log for a future CR rather than fixed here.
- README.md was 564 lines against this CR's 200-line ceiling.
- No CHANGELOG.md existed. No prior reconciliation marker, so this run's
  range is the entire project history.
- No `docs/` directory exists. Nothing to reconcile there.

## Unknowns

- Whether the palette.sh-copy defect (raw shell error, misleading exit code
  2) is worth its own CR, or small enough to fold into whatever CR next
  touches `delta/bin/verify`'s startup — a decision for whoever picks up the
  flagged item in CHANGELOG.md, not this one.
