# CR-002.R — Revert the runner machinery from CR-002

## Context

CR-002 solved a real problem — commands only worked where you had copied
them — and then kept going, solving a second, self-inflicted problem
(comparing a repo's runner against a machine-wide one) with state to track
state: a global runner copy, a version stamp, a drift comparison, a bootstrap
step. The runner living in the repo was always available and needed no code
at all. This reverts that machinery and keeps the actual fix.

## REMOVED

- `~/.delta/bin/verify` and `~/.delta/constitution-template.md` — `install`
  no longer writes anything under `~/.delta/`; nothing is written there at
  all.
- The runner-bootstrap step in `propose`'s lazy-init (copying
  `~/.delta/bin/verify` into a new repo's `delta/bin/verify`).
- The bootstrap-if-missing fallback in `verify`'s command file (copying from
  `~/.delta/bin/verify` if the repo's own copy was absent).
- `RUNNER_VERSION` in `delta/bin/verify`, the `runner_version:` line in
  `meta.txt`, and the version-mismatch note comparing a repo's runner against
  `~/.delta/bin/verify`.

## MODIFIED

- `delta/bin/install` writes only the five command files into each adapter's
  `user_dir`. Nothing else. It no longer touches `~/.delta/` at all.
- `propose`'s lazy-init step 3 no longer creates `delta/bin/verify`. It
  checks whether the file already exists and, if not, tells the user plainly
  to copy it in from another repo that already has delta — one file, one
  command — rather than fabricating its contents. Steps 1, 2, and 4
  (`truth/`, `changes/`, the constitution template) are unchanged and do not
  depend on the runner being present.
- `verify`'s command file locates the right `delta/bin/verify` by walking up
  and hands off to it; if it is missing, it says so and stops. It no longer
  copies anything into place.
- `delta/bin/verify` distinguishes a check that **could not run** (missing
  executable bit; a shebang naming an interpreter that is missing or not
  itself executable, detected via exit codes 126/127) from a criterion that
  **did not hold**. The former is a new `error` state, exit code **7**,
  counted separately and never folded into `failed` — reporting a
  configuration problem as a failed criterion sends someone to debug
  application code that is fine.
- `propose`'s check-writing instructions now say `chmod +x` explicitly,
  rather than "make it executable" in prose.

## ADDED

- `.gitattributes` forcing LF line endings on `delta/bin/**` and
  `**/checks/**` — a checkout with CRLF breaks every shebang in the tree,
  regardless of platform.
- Exit code 7 in `delta/bin/verify`, documented in its header alongside the
  existing 0–6, with precedence `7 > 1 > 6 > 2 > 3 > 5`.

## Out of scope

WSL is not recommended in favour of Git Bash as a platform note in this
change - the README's Windows guidance is updated, but no code branches on
platform. The runner already avoided every non-portable construct the
platform concern was about (`sed -i`, `readlink -f`, `date -d`, `grep -P`,
arrays, `[[`, process substitution) before this change; that was confirmed
by audit, not added here.

## Acceptance criteria

- C1 ~/.delta/ does not exist after install, and nothing recreates it
- C2 verify runs from the repo copy with no global installation of the runner
- C3 a fresh clone of a delta-using repo runs verify with no setup
- C4 commands still work in a repo that has never seen delta
- C5 no message anywhere refers to runner versions, drift, or updating
- C6 MANUAL a check that cannot run (no exec bit, or a missing/non-executable interpreter) is reported as a distinct error, exit 7, and never counted as a failed criterion
      reason: this spans rendering (a new glyph and summary line), the exit
      code table, and two distinct code paths (the pre-flight -x test and the
      126/127 codes after an attempted exec) - no single automated assertion
      covers "this is genuinely a different category from a failure" the way
      a human reading the actual terminal output and the header docs together
      can
      look at: run `delta/bin/verify` against a change with one check missing
      its executable bit and one check with a nonexistent interpreter in its
      shebang; read the header comment in delta/bin/verify documenting exit 7
      confirm: both render with the error glyph (not the fail glyph), neither
      is counted in "N failed" in the summary, the run exits 7 rather than 1,
      and the header documentation states the precedence and the rationale
