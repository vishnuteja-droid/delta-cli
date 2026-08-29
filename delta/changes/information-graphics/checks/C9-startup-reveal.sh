#!/bin/sh
# CRITERION: C9 the startup reveal plays once per terminal session, is skipped when piped, and is interruptible
set -eu
. "${DELTA_CHECK_DIR:-delta/changes/information-graphics/checks}/lib.sh"
have_python3 || { echo "no python3 on this machine"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mk_repo "$tmp/repo"
mk_change "$tmp/repo" x '- C1 quick'
add_check "$tmp/repo" x C1 'exit 0'

# Piped (no TTY): must never animate - a single instant frame, no carriage
# returns rewriting it.
piped_out=$(cd "$tmp/repo" && LANG=en_US.UTF-8 ./delta/bin/verify x 2>&1)
cr=$(printf '\r')
redraws=$(printf '%s' "$piped_out" | grep -aoE "${cr}  " | wc -l | tr -d ' ')
[ "$redraws" -le 1 ] || { echo "piped run redrew the opening frame $redraws times - should not animate at all"; exit 1; }

result_file="$tmp/result.txt"
python3 - "$tmp/repo" "$result_file" <<'PYEOF'
import pty, os, sys, time, select

repo, result_file = sys.argv[1], sys.argv[2]

def spawn(cmd, tmpdir):
    os.makedirs(tmpdir, exist_ok=True)
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(repo)
        env = dict(os.environ)
        env["LANG"] = "en_US.UTF-8"
        env["TMPDIR"] = tmpdir
        os.execvpe("sh", ["sh", "-c", cmd], env)
    return pid, fd

def drain(fd, pid, interrupt_after=None):
    out = b""
    if interrupt_after is not None:
        time.sleep(interrupt_after)
        os.write(fd, b'Q')
    deadline = time.time() + 5
    idle_since = None
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.3)
        if not r:
            if out and idle_since is None:
                idle_since = time.time()
            if idle_since and time.time() - idle_since > 0.5:
                break
            continue
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    return out

# Session A: two verify calls, same pty, same session - the marker keyed by
# `tty` should make only the first one animate.
pid, fd = spawn("./delta/bin/verify x; echo ---SPLIT---; ./delta/bin/verify x", repo + "/.reveal-tmp-a")
out = drain(fd, pid)
parts = out.split(b'---SPLIT---')
run1 = parts[0] if parts else b''
run2 = parts[1] if len(parts) > 1 else b''

# Session B: a fresh session, interrupted almost immediately - must still
# produce the closing frame instead of hanging.
pid2, fd2 = spawn("./delta/bin/verify x", repo + "/.reveal-tmp-b")
run3 = drain(fd2, pid2, interrupt_after=0.05)

redraws1 = run1.count(b'\r  ')
redraws2 = run2.count(b'\r  ')

with open(result_file, "w") as f:
    f.write("redraws1=%d redraws2=%d run3_len=%d\n" % (redraws1, redraws2, len(run3)))
PYEOF

[ -f "$result_file" ] || { echo "python3 pty test produced no result"; exit 1; }
cat "$result_file"
r1=$(sed -n 's/.*redraws1=\([0-9]*\).*/\1/p' "$result_file")
r2=$(sed -n 's/.*redraws2=\([0-9]*\).*/\1/p' "$result_file")
r3len=$(sed -n 's/.*run3_len=\([0-9]*\).*/\1/p' "$result_file")

[ "$r1" -gt 1 ] || { echo "run 1 (first in this session) did not animate at all"; exit 1; }
[ "$r2" -le 1 ] || { echo "run 2 (same session) animated again - the once-per-session marker did not hold"; exit 1; }
[ "$r3len" -gt 0 ] || { echo "an interrupted reveal produced no output - the run hung or was killed outright"; exit 1; }

exit 0
