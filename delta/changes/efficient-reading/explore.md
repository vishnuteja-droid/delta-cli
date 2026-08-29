# Findings — CR-007 efficient reading

Read: delta/commands/{explore,propose,apply,archive}.md, delta/truth/
(empty before this change except a placeholder README), CHANGELOG.md's
existing entries for the established prose style.

## Entry points

- The five command prompts themselves — this CR only changes prose, no code.

## Data touched

- `delta/commands/explore.md`, `propose.md`, `apply.md`, `archive.md`
  (source of truth for delta's own five per-CLI generated files each).

## Unknowns

- Whether real agent runs (outside this session) will actually follow the
  new instructions as faithfully as the dogfooding below did — a prompt
  changing behaviour is inherently probabilistic in a way a script's
  behaviour is not. Mitigated by writing the instructions as concretely and
  mechanically as possible (literal git commands, a numbered ladder, a
  named stop condition) rather than as vague guidance.

## Real dogfooding, not assertions

This CR's acceptance criteria describe agent behaviour, which cannot be
verified by a script the way delta/bin/verify's own checks can. Two real
exercises instead of bare claims:

**Truth-first reading, in this repository.** `docs-reconciliation` was
archived into `delta/truth/docs.md` specifically to make this testable
against real committed history (see the `archive:` commit, d21cf69). The
*first* explore of this "docs" area happened during the `docs-reconciliation`
CR itself, before truth existed: README.md, AGENTS.md, delta/constitution.md,
all five `delta/commands/*.md`, five `delta/bin/*` header comments, and
`delta/adapters.yaml` — 13 files, no truth to consult. A *second* explore of
the same area, now, following the new instructions: `git log -1 -- delta/
truth/docs.md` (one commit, d21cf69), then `git log --oneline d21cf69..HEAD
-- README.md AGENTS.md CHANGELOG.md delta/changes/docs-reconciliation/`
(empty — nothing has changed). Truth answers everything; the second explore
opens exactly one file, `delta/truth/docs.md` itself. 13 files to 1.

**Stale-truth detection and service-boundary bounding, on a fresh fixture**
(not this repo — a scratch fixture makes a deliberately-wrong truth claim
safe to construct without polluting real truth): `delta/truth/payments.md`
claimed dispatch "posts synchronously... and waits for the response". The
real code (`src/papi/dispatch.py`) calls `ledger_client.publish_async` and
returns immediately. Following the new instructions produced a findings
file that: named the stale entry explicitly (code wins, truth said sync,
code is fire-and-forget); treated `ledger_client` — otherservice's own
client library, an external import with no local source — as an edge and
did not go looking for otherservice's own code; and never opened
`dispatch_test.py`, a test file not named in the intent. The fixture (truth
before, source, and the findings file that run actually produced) is kept
in this change's `dogfood/` directory as evidence, not summary.

Both exercises are cited, with real file counts and real output, in this
change's MANUAL sign-offs rather than asserted.
