#!/usr/bin/env python3
# Measures a change's page against its base's, each served on its own port,
# in a fresh Chrome a run, base and change by turns, spoken to over Chrome's
# debugging pipe. Read off the app's own profiler (`pxl.perf.read`,
# `pxl.perf.drive`): the first load's longest frame, which its long-frame
# observer replays from load (perf/long-frame.ts), and the GPU's busy time and
# the core's tick over the one measurement (perf/drive.ts). Fails where the
# change's median runs past the base's: its load's longest frame by more than
# `LOAD_SLACK_MS`, its GPU's or its tick's p50 or p95 by more than `P50_SHARE`
# or `P95_SHARE`, its tick also by more than `TICK_SLACK_MS`.
#
#   ./script/compare-perf.py <base url> <change url> [rounds]

import json
import math
import os
import select
import shutil
import subprocess
import sys
import tempfile
import time

CHROME = os.environ.get(
    "CHROME", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
)
# 4K, at the two device pixels a side below.
SIZE = {"width": 1920, "height": 1080}
# Where the device stands, so the view opens there as a user's does: on
# Seoraksan, whose ridges the drive's eye meets at the horizon.
PLACE = {"latitude": 38.1193, "longitude": 128.4656}
# How much longer the change's first load may block the page than the base's.
LOAD_SLACK_MS = 3000
# How much more the change's GPU time may take than the base's.
P50_SHARE = 1.15
P95_SHARE = 1.3
# A tick's share is of little time: what it may run past that as well.
TICK_SLACK_MS = 0.5
# The page settled: no resource ends for this long, within `LOAD_CAP_S`.
QUIET_S = 8
LOAD_CAP_S = 90
# The longest the page may take to answer one ask, a drive included.
EVAL_CAP_S = 120
DRIVE_S = 20

READ = """(async () => {
  const r = await pxl.perf.read();
  const f = r.frames;
  return {
    load: Math.max(0, ...r.longs.map((l) => l.durationMs)),
    gpu: f ? [f.gpu_busy.p50, f.gpu_busy.p95, f.n, f.gpu_read] : [null, null, 0, 0],
    tick: f ? [f.tick.p50, f.tick.p95] : [null, null],
  };
})()"""
LAST_END = "Math.max(0, ...performance.getEntriesByType('resource').map((e) => e.responseEnd))"


class Page:
    """A fresh Chrome on `url`: its commands go in on the pipe it reads as
    fd 3, its answers come back on the one it writes as fd 4, each message
    ended by a NUL."""

    def __init__(self, url):
        self.dir = tempfile.mkdtemp(prefix="pxl-compare-perf-")
        to_r, self.to = os.pipe()
        self.of, of_w = os.pipe()

        def pipes():
            os.dup2(to_r, 100)
            os.dup2(of_w, 101)
            os.dup2(100, 3)
            os.dup2(101, 4)

        self.chrome = subprocess.Popen(
            [
                CHROME,
                f"--user-data-dir={self.dir}",
                "--remote-debugging-pipe",
                "--no-first-run",
                "--no-default-browser-check",
                # A window behind another draws as one in front: no frame is throttled.
                "--disable-backgrounding-occluded-windows",
                "--disable-renderer-backgrounding",
                "--disable-background-timer-throttling",
                # A CSS pixel is two device pixels on whichever screen the
                # window opens: every run draws twice `SIZE`'s pixels a side.
                "--force-device-scale-factor=2",
                "about:blank",
            ],
            preexec_fn=pipes,
            close_fds=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        os.close(to_r)
        os.close(of_w)
        self.buf = b""
        self.id = 0
        self.session = None
        # A page of its own: the one Chrome opens with is replaced as it
        # navigates, and its session with it.
        page = self.send("Target.createTarget", {"url": "about:blank"})["targetId"]
        self.session = self.send(
            "Target.attachToTarget", {"targetId": page, "flatten": True}
        )["sessionId"]
        self.send(
            "Emulation.setDeviceMetricsOverride",
            {**SIZE, "deviceScaleFactor": 0, "mobile": False},
        )
        # The device stands at `PLACE`, so the view opens there as a user's does.
        origin = "/".join(url.split("/")[:3])
        self.send(
            "Browser.grantPermissions",
            {"origin": origin, "permissions": ["geolocation"]},
            browser=True,
        )
        self.send("Emulation.setGeolocationOverride", {**PLACE, "accuracy": 10})
        # Every resource kept, past the browser's 250, so the load is seen to
        # settle; a script for new documents runs only with the page's events on.
        self.send("Page.enable")
        self.send(
            "Page.addScriptToEvaluateOnNewDocument",
            {"source": "performance.setResourceTimingBufferSize(1e6)"},
        )
        self.send("Page.navigate", {"url": url})

    def send(self, method, params=None, browser=False):
        """`method`'s answer, the page's unless `browser` or no page is
        attached yet; an error, or none within `EVAL_CAP_S`, raises."""
        self.id += 1
        msg = {"id": self.id, "method": method, "params": params or {}}
        if self.session and not browser:
            msg["sessionId"] = self.session
        os.write(self.to, json.dumps(msg).encode() + b"\0")
        end = time.monotonic() + EVAL_CAP_S
        while True:
            m = self.next(end, method)
            if m.get("id") == self.id:
                if "error" in m:
                    raise RuntimeError(f"{method}: {m['error'].get('message')}")
                return m["result"]

    def next(self, end, method):
        while b"\0" not in self.buf:
            left = end - time.monotonic()
            if left <= 0 or not select.select([self.of], [], [], left)[0]:
                raise TimeoutError(f"no answer in {EVAL_CAP_S} s: {method}")
            chunk = os.read(self.of, 1 << 20)
            if not chunk:
                raise RuntimeError(f"Chrome closed: {method}")
            self.buf += chunk
        m, _, self.buf = self.buf.partition(b"\0")
        return json.loads(m)

    def ev(self, expression):
        r = self.send(
            "Runtime.evaluate",
            {"expression": expression, "awaitPromise": True, "returnByValue": True},
        )
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"]["text"])
        return r["result"].get("value")

    def close(self):
        self.chrome.terminate()
        try:
            self.chrome.wait(10)
        except subprocess.TimeoutExpired:
            self.chrome.kill()
            self.chrome.wait()
        os.close(self.to)
        os.close(self.of)
        shutil.rmtree(self.dir, ignore_errors=True)


def once(url):
    """One page: its load's longest frame and its GPU busy time. A
    page a run, so each run's load is a first load."""
    page = Page(url)
    try:
        last, quiet, t = 0, 0, 0
        while quiet < QUIET_S and t < LOAD_CAP_S:
            time.sleep(1)
            t += 1
            try:
                end = page.ev(LAST_END) or 0
            except RuntimeError:
                end = 0
            quiet = 0 if end > last else quiet + 1
            last = max(last, end)
        # The drive turns the profiler on, which installs the long-frame
        # observers, and they replay the load.
        page.ev(f"pxl.perf.drive({DRIVE_S})")
        return page.ev(READ)
    finally:
        page.close()


def median(v):
    """A figure the window could not give (None) counts as none: NaN fails
    every test."""
    v = [math.nan if x is None else x for x in v]
    if any(math.isnan(x) for x in v):
        return math.nan
    return sorted(v)[(len(v) - 1) // 2]


def main():
    if len(sys.argv) < 3:
        print("compare-perf.py <base url> <change url> [rounds]", file=sys.stderr)
        return 2
    base, change = sys.argv[1:3]
    rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    runs = {"base": [], "change": []}
    for _ in range(rounds):
        for who, url in (("base", base), ("change", change)):
            runs[who].append(once(url))
    if os.environ.get("COMPARE_RUNS"):
        print(json.dumps(runs))
    fails = []
    load = [
        median([r["load"] for r in runs[who]])
        for who in ("base", "change")
    ]
    print(f"first load, longest frame ms: {load[0]:.0f} -> {load[1]:.0f}")
    if not load[1] <= load[0] + LOAD_SLACK_MS:
        fails.append(f"the first load blocks {load[1]:.0f} ms")
    b, c = (
        [median([r["gpu"][k] for r in runs[who]]) for k in range(4)]
        for who in ("base", "change")
    )
    if not (b[3] > 0 and c[3] > 0):
        fails.append("no GPU frame timed")
    print(
        f"GPU p50 {b[0]:.2f} -> {c[0]:.2f}, p95 {b[1]:.2f} -> {c[1]:.2f}, "
        f"frames {b[2]} -> {c[2]}, timed {b[3]} -> {c[3]}"
    )
    if not c[0] <= b[0] * P50_SHARE:
        fails.append(f"GPU p50 past {P50_SHARE}x the base's")
    if not c[1] <= b[1] * P95_SHARE:
        fails.append(f"GPU p95 past {P95_SHARE}x the base's")
    b, c = (
        [median([r["tick"][k] for r in runs[who]]) for k in range(2)]
        for who in ("base", "change")
    )
    print(f"tick p50 {b[0]:.2f} -> {c[0]:.2f}, p95 {b[1]:.2f} -> {c[1]:.2f}")
    if not c[0] <= b[0] * P50_SHARE + TICK_SLACK_MS:
        fails.append(f"tick p50 past {P50_SHARE}x the base's")
    if not c[1] <= b[1] * P95_SHARE + TICK_SLACK_MS:
        fails.append(f"tick p95 past {P95_SHARE}x the base's")
    for f in fails:
        print(f"FAIL {f}")
    print("fail" if fails else "pass")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
