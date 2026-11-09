#!/usr/bin/env python3
# One measurement in desktop Chrome, a fresh profile each run.
#
#   ./script/perf/chrome.py <url> <dir>

import json
import os
import shutil
import subprocess
import tempfile

from cdp import Cdp
from measure import ROOT, SIZE, measure, polled

CHROME = os.environ.get(
    "CHROME", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
)
# Where each run's Chrome keeps its profile, removed when the run ends.
TMP = os.path.join(ROOT, "tmp")


class Page(Cdp):
    """A fresh Chrome on `url`: its commands go in on the pipe it reads as
    fd 3, its answers come back on the one it writes as fd 4, each message
    ended by a NUL."""

    def __init__(self, url):
        os.makedirs(TMP, exist_ok=True)
        self.dir = tempfile.mkdtemp(prefix="perf-chrome-", dir=TMP)
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
        self.send("Page.navigate", {"url": url})

    def put(self, message):
        os.write(self.to, message + b"\0")

    def get(self):
        while b"\0" not in self.buf:
            chunk = os.read(self.of, 1 << 20)
            if not chunk:
                raise RuntimeError("Chrome closed")
            self.buf += chunk
        m, _, self.buf = self.buf.partition(b"\0")
        return json.loads(m)

    def close(self):
        self.chrome.terminate()
        self.chrome.wait()
        os.close(self.to)
        os.close(self.of)
        shutil.rmtree(self.dir, ignore_errors=True)


def chrome(url):
    page = Page(url)
    try:
        yield from polled(page.ev)
    finally:
        page.close()


measure(chrome)
