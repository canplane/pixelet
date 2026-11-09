# What every vendor's measurement shares. A vendor's script starts a build made
# for measuring (`PXL_PERF_DRIVE=1`) from nothing and hands `measure` the lines
# it says; `measure` writes the result into <dir>: values.json, report.txt and
# trace.json, which ui.perfetto.dev opens. The scenario and the values are the
# app's (ts/client/src/ui/debug/perf/measure.ts).

import json
import os
import re
import signal
import subprocess
import sys
import time
from contextlib import closing

# The longest a run may take, its install included.
CAP_S = 240
PIECE = re.compile(r"PXLPERF (\d+)/(\d+) (.*)")
# The most lines read from a page at once.
BATCH = 30
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# A page's size: 4K, at Chrome's two device pixels a side.
SIZE = {"width": 1920, "height": 1080}


def polled(ev):
    """The lines a page has said, as it says them, `BATCH` to an answer, as one
    answer of a whole result does not come back from a phone's Chrome. A page
    between documents answers nothing."""
    seen = 0
    while True:
        try:
            said = ev(f"(globalThis.said ?? []).slice({seen}, {seen + BATCH})")
        except RuntimeError:
            said = []
        yield from said
        seen += len(said)
        if len(said) < BATCH:
            time.sleep(1)


def stream(cmd):
    """The lines `cmd` writes, until the reader lets go."""
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True, errors="replace")
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


def measure(start):
    """One run: `start(target)` gives the lines, and letting go of them ends
    what it started."""
    if len(sys.argv) != 3:
        print(f"{os.path.basename(sys.argv[0])} <target> <dir>", file=sys.stderr)
        sys.exit(2)
    target, out = sys.argv[1:]

    def expired(*_):
        raise TimeoutError(f"no result in {CAP_S} s")

    signal.signal(signal.SIGALRM, expired)
    signal.alarm(CAP_S)
    with closing(start(target)) as lines:
        run = json.loads(told(lines))
    if "error" in run:
        sys.exit(f"{target}: {run['error']}")
    report, trace = run.pop("report"), run.pop("trace")
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "values.json"), "w") as f:
        json.dump(run, f)
    with open(os.path.join(out, "report.txt"), "w") as f:
        f.write(report)
    with open(os.path.join(out, "trace.json"), "w") as f:
        json.dump(trace, f)
    print(f"{run['build']}: {out}")
