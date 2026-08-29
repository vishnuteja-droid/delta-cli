# Docs

`README.md` is the entry point: under 200 lines, gets a newcomer to a
working `delta/bin/install` within the first third, states the Windows/Git
Bash requirement and the deliberately-not-built list. Depth that does not
fit stays in the source files' own header comments (`delta/bin/verify`,
`delta/bin/palette.sh`, `delta/bin/report`) rather than a `docs/` directory,
which does not exist and is not created to hold overflow.

`AGENTS.md` carries the lifecycle and `delta/bin/verify`'s exit codes in
about the same handful of lines, read natively by tools that support the
AGENTS.md convention without a per-CLI command file.

`CHANGELOG.md` has one entry per shipped change, newest first, each with an
explicit `behaviour change: yes/no` line. A `## Reconciliation log` section
at the bottom records each `docs-reconciliation` run's date and findings.

`delta/changes/docs-reconciliation/` is a recurring change, not a one-shot:
its checks (grep-based, real fixtures) are meant to be re-run and extended
on every pass, not replaced. Two criteria stay MANUAL permanently — whether
the shown examples actually run, and whether the README still reads well —
since neither is mechanically checkable. Reconciliation is considered
overdue past 4 shipped changes since the last dated marker.
