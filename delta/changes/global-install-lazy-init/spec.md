# CR-002 — Global commands, lazy per-repo data

## Context

delta shipped as a folder copied into each repository — commands, runner, and
data together. That made adoption a per-repo chore and meant the commands
were unavailable anywhere they had not been set up. It blocked the common
case: you are in a repo, something is confusing, and you want
`/delta:explore` right now, with nothing to install first.

Commands are identical everywhere and belong to the developer. Truth,
changes, and the constitution describe one codebase and belong to the repo.
Those two halves had one home; now they have two.

## ADDED

- `delta/bin/install` — writes the five command files into every adapter's
  `user_dir` (from `adapters.yaml`), copies `delta/bin/verify` to
  `~/.delta/bin/verify`, and copies `delta/constitution.md` to
  `~/.delta/constitution-template.md`. Run once per machine; every repo on it
  gets the commands. Copies files that already exist locally - no network
  fetch, no package manager.
- `adapters.yaml` gained a `user_dir` field per adapter alongside the
  existing `dir` (project-level) field.
- `generate-commands --target user|project` — `project` is the existing,
  default, committed-to-the-repo behaviour. `user` writes to each adapter's
  `user_dir` under `$HOME` instead; `install` calls this internally.
- `generate-commands` gained `inject_constitution`, which splices
  `delta/constitution.md` verbatim into any generated command body wherever
  the source contains a line reading `{{CONSTITUTION_TEMPLATE}}`. There is
  exactly one canonical copy of the template; the copy every CLI ships can
  never drift from it, because it is not a copy at generation time - it is
  spliced in.
- `delta/bin/verify` honours `DELTA_ROOT` as an explicit override of root
  discovery, and announces the resolved root (`root: /path/to/repo`) whenever
  it differs from the current directory.
- `delta/bin/verify` notes when `~/.delta/bin/verify` carries a higher
  `RUNNER_VERSION` than the repo's own copy. It never updates itself.
- `propose` creates `delta/{truth,changes,bin}` and a `constitution.md`
  template - not generated description - the first time it runs in a repo
  that has never seen delta. No separate init command exists or is planned.
- `propose`'s lazy-init step also bootstraps `delta/bin/verify` into the new
  `delta/bin/`, copied from `~/.delta/bin/verify`, and tells the user to
  commit it.
- `verify`'s command file now bootstraps `delta/bin/verify` into the repo
  from `~/.delta/bin/verify` if it is missing (a defensive fallback - the
  normal path is that `propose` already put it there), before invoking it.
- `explore` works in a repository that has never run `propose`: it resolves
  the root by walking up for `delta/`, then for `.git`; when only `.git` is
  found, it writes no files and prints its findings to the terminal in full.

## MODIFIED

- `apply` and `archive` now resolve their root by walking up for `delta/`
  from the current directory (honouring `DELTA_ROOT`) rather than assuming
  the current directory is the root. Both stop with a clear message if no
  `delta/` is found anywhere above them.
- `archive`'s "run verify first" step names the resolved root's runner
  explicitly rather than assuming `delta/bin/verify` is reachable from cwd.

## REMOVED

None.

## RENAMED

None.

## Out of scope

No package manager, no installer beyond a shell script, no network fetch -
`install` copies files that already exist locally. No global state beyond
`~/.delta/`; nothing about a repo is ever recorded outside that repo.

## Acceptance criteria

- C1 delta/bin/install writes only under $HOME; the repository it is run from is never touched
- C2 the constitution template embedded in propose is identical to delta/constitution.md, and still reads as a template
- C3 delta/bin/verify resolves its root by walking up for delta/, and announces the root when it differs from cwd
- C4 DELTA_ROOT overrides discovery
- C5 project-level and user-level command output always land at different absolute paths
- C6 a repo with delta/bin/verify committed runs correctly with no ~/.delta present
- C7 verify notes a version mismatch against a newer global runner, and never modifies the repo's own runner
- C8 MANUAL explore, propose, apply, and archive each correctly instruct root discovery, and propose's lazy-init instructs creating delta/ from the template with no invented content and no runner fabricated by hand
      reason: these are agent-executed prompts; correctness is a property of
      the instruction text, which only a read can judge, not a check that runs
      the prompt as code
      look at: delta/commands/explore.md, propose.md, apply.md, archive.md -
      specifically each "## Where this repository's delta/ lives" section,
      and propose.md's "## Creating delta/ for the first time" section
      confirm: each names DELTA_ROOT as the override, walks up for delta/
      (then .git where relevant), announces a resolved root that differs from
      cwd, and propose's lazy-init copies the constitution template verbatim
      with an explicit instruction never to fill it in or introspect the repo,
      and bootstraps the runner by file copy only, refusing to fabricate one
