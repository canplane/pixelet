#!/usr/bin/env python3
# One measurement in desktop Chrome, a fresh profile each run.
#
#   ./script/perf/chrome.py <url> <dir>

import json
import os
import re
import shutil
import subprocess
import tempfile

import mac
from cdp import Cdp
from measure import ROOT, SIZE, Cpu, Tail, measure, polled

CHROME = os.environ.get(
    "CHROME", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
)
# Where each run's Chrome keeps its profile, removed when the run ends.
TMP = os.path.join(ROOT, "tmp")
# A line Chrome's log writes from the host of its GPU process: the process
# exiting, crashing or being given up.
GPU_PROCESS = re.compile(r"gpu_process_host")
HELPER = re.compile(r" --type=(\S+)")
COMPILER = re.compile(r"/MTLCompilerService(?: |$)")


class Page(Cdp):
    """A fresh Chrome on `url`, a reach (measure.py): its commands go in on
    the pipe it reads as fd 3, its answers come back on the one it writes as
    fd 4, each message ended by a NUL. The state is the Mac's, then how many
    lines its log wrote of its GPU process (`gpu_process_host`) since the last
    capture, and the last of them, then the CPU its page, its GPU process and
    the Metal compiler took."""

    @staticmethod
    def running():
        """A fresh profile's Chrome runs apart from any other, its GPU process
        its own, so nothing running is shared."""
        return None

    def __init__(self, url):
        self.cpu = Cpu(self.role)
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
                # A CSS pixel is two device pixels on whichever screen the
                # window opens: every run draws twice `SIZE`'s pixels a side.
                "--force-device-scale-factor=2",
                "--enable-logging=stderr",
                "about:blank",
            ],
            preexec_fn=pipes,
            close_fds=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            errors="replace",
        )
        self.gpu = Tail(self.chrome.stderr, GPU_PROCESS)
        os.close(to_r)
        os.close(of_w)
        self.buf = b""
        try:
            # A page of its own: the one Chrome opens with is replaced as it
            # navigates, and its session with it.
            page = self.send("Target.createTarget", {"url": "about:blank"})[
                "targetId"
            ]
            self.session = self.send(
                "Target.attachToTarget", {"targetId": page, "flatten": True}
            )["sessionId"]
            self.send(
                "Emulation.setDeviceMetricsOverride",
                {**SIZE, "deviceScaleFactor": 0, "mobile": False},
            )
            self.send("Page.navigate", {"url": url})
        except BaseException:
            self.close()
            raise
        self.lines = polled(self.ev)

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

    def role(self, ppid, cmd):
        """This Chrome's renderers run the page and its GPU process is its
        own, each by the `--type` its helper is started with, whatever the
        helper is named (measure.py `Cpu`)."""
        if COMPILER.search(cmd):
            return "compiler"
        if ppid != self.chrome.pid:
            return None
        kind = HELPER.search(cmd)
        return {"renderer": "page", "gpu-process": "gpu"}.get(kind[1]) if kind else None

    def state(self):
        lines = self.gpu.take()
        log = f"gpu_process_lines {len(lines)} {lines[-1] if lines else ''}".rstrip()
        return f"{mac.state()}; {log}; {self.cpu.take()}"

    def close(self):
        self.chrome.terminate()
        self.chrome.wait()
        os.close(self.to)
        os.close(self.of)
        shutil.rmtree(self.dir, ignore_errors=True)


# The run holds its device from before it starts (`measure.hold`).
mac.hold()
measure(Page)
