# What every vendor's measurement shares. A vendor's script starts a build made
# for measuring (`PXL_PERF_DRIVE=1`) from nothing and hands `measure` how it
# reaches it; `measure` writes the result into <dir>: values.json, report.txt and
# trace.json, which ui.perfetto.dev opens. Beside them, however the run ends, it
# keeps log.txt, every other line the app wrote and each capture's, and shots/,
# the screen every `CAPTURE_S`. The scenario, the values and the progress are
# the app's (ts/client/src/ui/feature/debug/perf/measure.ts).
#
# A run starts its browser or app afresh, so that nothing a run before left
# shares its process or its GPU's, and starts none where the vendor says one is
# running: what it would share. A vendor's reach is made by the vendor's
# `start(target)`, which starts it afresh, and `start.running()` says what is
# running, or None. The reach has `lines`, what the app says; `shot(path)`,
# which writes a PNG of the screen to `path`; `state()`, the device's state as
# a line, how busy its GPU is first; and `close()`, which ends what
# the script started. `shot` and `state` are called from a thread of their
# own, beside `lines`.
#
# How a vendor starts afresh and what its state reads are what its platform
# reaches, and differ only so: a Mac browser's state adds its GPU process's
# log and the CPU each of its processes took, a phone's its GPU's clock and
# heat, and an iPhone's nothing, as devicectl and safaridriver read none.

import fcntl
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time

# The longest a run may take, its install included.
CAP_S = 240
# The longest a reach may take to start, an app's install included: a browser
# that gives no session or no page by then ends the run, saying so.
START_S = 120
PIECE = re.compile(r"PXLPERF (\d+)/(\d+) (.*)")


_held = {}


def hold(device):
    """Waits until this process holds `device`, and holds it until the process
    ends. One user drives a device at a time across every checkout of the
    repository: a run, the shader probe, and on the Mac every Rust test that
    opens its GPU (pxl-gpu-host lock.test.rs). The lock is
    `tmp/lock/<device>.lock` in the main checkout, which its worktrees share."""
    if device in _held:
        return
    common = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=os.path.dirname(os.path.abspath(__file__)),
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    path = os.path.join(os.path.dirname(common), "tmp", "lock", f"{device}.lock")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    f = open(path, "a")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print(f"waiting for {device}, held at {path}", file=sys.stderr)
        fcntl.flock(f, fcntl.LOCK_EX)
    _held[device] = f


def holding(hold):
    """Runs the command in argv holding a device (`hold`), and exits as it
    does: a device's module run as a script. The lock is not handed to the
    command, so no process it leaves running keeps it."""
    hold()
    sys.exit(subprocess.run(sys.argv[1:], close_fds=False).returncode)


class Expired(Exception):
    """A limit of the run's ran out. It is no OSError, so no wait a vendor
    retries on a failed connection takes it for one."""
# The most lines read from a page at once.
BATCH = 30
# The seconds between captures of the screen and the device's state.
CAPTURE_S = 10
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# A page's size: 4K, at Chrome's two device pixels a side.
SIZE = {"width": 1920, "height": 1080}


def polled(ev):
    """The lines a page writes, as it writes them, `BATCH` to an answer, as one
    answer of a whole result does not come back from a phone's Chrome. A page
    between documents answers nothing."""
    seen = 0
    while True:
        try:
            lines = ev(f"(globalThis.lines ?? []).slice({seen}, {seen + BATCH})")
        except RuntimeError:
            lines = []
        yield from lines
        seen += len(lines)
        if len(lines) < BATCH:
            time.sleep(1)


def stream(cmd):
    """The lines `cmd` writes, to its output and its errors alike, until the
    reader lets go."""
    p = subprocess.Popen(
        cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace"
    )
    try:
        yield from p.stdout
    finally:
        p.terminate()
        p.wait()


def told(lines):
    """The one text the app tells in numbered pieces among `lines`."""
    pieces = {}
    for line in lines:
        m = PIECE.search(line)
        if m:
            pieces[int(m[1])] = m[3]
            if len(pieces) == int(m[2]):
                return "".join(pieces[i + 1] for i in range(len(pieces)))
    raise RuntimeError("the output ended before the app had told its result")


class Tail:
    """The lines `stream` gives that `keep` finds, read in a thread of their
    own as they come; `take()` hands each once."""

    def __init__(self, stream, keep):
        self.lock = threading.Lock()
        self.kept = []

        def read():
            # The cap's alarm is the main thread's to take.
            signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGALRM})
            for line in stream:
                if keep.search(line):
                    with self.lock:
                        self.kept.append(line.rstrip())

        threading.Thread(target=read, daemon=True).start()

    def take(self):
        with self.lock:
            kept, self.kept = self.kept, []
        return kept


def processes():
    """Every process as (pid, parent's pid, CPU seconds so far, command line)."""
    ps = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,time=,args="], stdout=subprocess.PIPE, text=True
    ).stdout
    for line in ps.splitlines():
        pid, ppid, cpu, cmd = line.split(None, 3)
        days, _, clock = cpu.rpartition("-")
        secs = 0.0
        for part in clock.split(":"):
            secs = secs * 60 + float(part)
        yield int(pid), int(ppid), secs + 86400 * int(days or 0), cmd


class Cpu:
    """The CPU each of a vendor's roles took since the last `take`, as a
    share of one core: `page` the processes that run the page, `gpu` its GPU
    process, `compiler` the Metal shader compiler. `role(ppid, cmd)` names a
    process's role, or None. A process that ran as the run began is another
    app's and is left out; one of the same kind another app starts meanwhile is
    counted, and what a process took between its last take and its end is
    not."""

    ROLES = ("page", "gpu", "compiler")

    def __init__(self, role):
        self.role = role
        self.before = {pid for pid, *_ in processes()}
        self.at = time.monotonic()
        self.was = {}

    def take(self):
        now, took, seen = time.monotonic(), dict.fromkeys(self.ROLES, 0.0), {}
        for pid, ppid, secs, cmd in processes():
            role = self.role(ppid, cmd)
            if role is None or pid in self.before:
                continue
            seen[pid] = secs
            took[role] += secs - self.was.get(pid, 0.0)
        span, self.at, self.was = now - self.at, now, seen
        return "cpu " + " ".join(f"{r} {100 * s / span:.0f}%" for r, s in took.items())


def measure(start):
    """One run: `start(target)` gives the vendor's reach, started afresh, and
    closing it ends what it started. Why a run ended is noted before what it
    started is closed, so a close that fails after it hides nothing."""
    if len(sys.argv) != 3:
        print(f"{os.path.basename(sys.argv[0])} <target> <dir>", file=sys.stderr)
        sys.exit(2)
    target, out = sys.argv[1:]
    running = start.running()
    if running is not None:
        sys.exit(f"{running}, and a run starts it afresh: close it first")

    limit = f"not started in {START_S} s"

    def expired(*_):
        raise Expired(limit)

    os.makedirs(os.path.join(out, "shots"), exist_ok=True)
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(START_S)
    with open(os.path.join(out, "log.txt"), "w") as log:
        t0 = time.monotonic()
        lock = threading.Lock()

        def note(line):
            """Keeps `line` in log.txt, after the run's seconds so far."""
            with lock:
                if not log.closed:
                    log.write(f"{time.monotonic() - t0:.1f} {line.rstrip()}\n")
                    log.flush()

        reach = None
        try:
            reach = start(target)
            limit = f"no result in {CAP_S} s"
            signal.alarm(max(1, round(CAP_S - (time.monotonic() - t0))))
            run = heard(reach, out, note, t0)
        except BaseException as e:
            note(f"ended: {e!r}")
            raise
        finally:
            if reach is not None:
                try:
                    reach.close()
                except BaseException as e:
                    note(f"close failed: {e!r}")
                    raise
    if "error" in run:
        sys.exit(f"{target}: {run['error']}")
    report, trace = run.pop("report"), run.pop("trace")
    with open(os.path.join(out, "values.json"), "w") as f:
        json.dump(run, f)
    with open(os.path.join(out, "report.txt"), "w") as f:
        f.write(report)
    with open(os.path.join(out, "trace.json"), "w") as f:
        json.dump(trace, f)
    print(f"{run['build']}: {out}")


def heard(reach, out, note, t0):
    """The result `reach`'s lines tell, every other line noted as it comes and
    the screen and the state captured beside them until it is told."""
    ended = threading.Event()

    def capture():
        # The cap's alarm is the main thread's to take.
        signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGALRM})
        while not ended.wait(CAPTURE_S):
            name = f"shots/{time.monotonic() - t0:04.0f}.png"
            try:
                reach.shot(os.path.join(out, name))
                note(f"shot {name}")
            except Exception as e:
                note(f"shot failed: {e!r}")
            try:
                note(f"state {reach.state()}")
            except Exception as e:
                note(f"state failed: {e!r}")

    def lines():
        for line in reach.lines:
            if not PIECE.search(line):
                note(line)
            yield line

    capturing = threading.Thread(target=capture, daemon=True)
    capturing.start()
    try:
        return json.loads(told(lines()))
    finally:
        ended.set()
        capturing.join(CAPTURE_S)
