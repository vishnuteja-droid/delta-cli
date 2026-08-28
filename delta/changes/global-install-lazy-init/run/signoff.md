# Manual criteria sign-off

One line per resolved MANUAL criterion:
    C<n> signed-off-by: <who> <date> - <what they saw>

verify greps for the criterion id at the start of a line. An unsigned MANUAL
criterion makes verify exit 3; it is never auto-passed.

C8 signed-off-by: build 2026-08-28 - re-read after CR-002.R revised propose's
lazy-init step 3. The "## Where this repository's delta/ lives" sections in
explore, propose, apply, and archive still each name $DELTA_ROOT as the
override, walk the current directory then each parent for delta/ (explore
and propose additionally fall back to walking for .git), and announce
`root: /path/to/repo` when the resolved root differs from cwd - unchanged by
the revert. Step 3 of propose's lazy-init now reads: check whether
delta/bin/verify already exists; never create it; there is no global copy to
pull from, delta/bin/verify is committed per-repo and that copy is the only
one that ever exists; if missing, tell the user plainly to copy it in from
another repo (a two-line cp + chmod +x example is shown) rather than
fabricate its contents; continue creating truth/, changes/, and the
constitution regardless, since none of those depend on the runner being
present. All conditions in the criterion's confirm list are met against the
current text.

Previously noted, still true and non-blocking: the embedded constitution's
code fence is not indented to match its parent numbered-list item.
