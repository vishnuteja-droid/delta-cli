# AGENTS.md

## Changing this system

Changes to existing behaviour go through delta, a spec lifecycle that lives in
`delta/`. Nothing needs installing; it is files plus one POSIX `sh` script.

1. **explore** — read the affected code and write findings to
   `delta/changes/<id>/explore.md`, including what could not be determined.
2. **propose** — write `spec.md` as a delta against `delta/truth/` using
   ADDED / MODIFIED / REMOVED / RENAMED, plus one executable check per
   acceptance criterion in `checks/`. Both need human approval before landing.
3. **apply** — implement the spec in order. Never edit a check to match the
   implementation.
4. **verify** — run `delta/bin/verify <change-id>`.
5. **archive** — fold the applied delta into `delta/truth/`.

`delta/constitution.md` is inherited by every change. Read it first.

## When to run verify

Run `delta/bin/verify` before claiming any change is complete, and again after
any change to the checks or the spec. It exits **0** all passed, **1** a check
failed, **2** a criterion has no check, **3** a MANUAL criterion is unsigned,
**6** a reproduction did not reproduce. Only 0 means done. A non-zero exit is
not a caveat to explain around.

For a bug fix, write the reproduction first and mark its check
`# EXPECT: fail-until-fixed`. It is expected to fail until the fix lands, so a
run reports it as `reproduced` and still exits 0. `archive` runs
`delta/bin/verify --archive-gate`, which exits **5** while any reproduction is
still outstanding.
