#!/usr/bin/env python3
# One measurement in the Mac's Safari.
#
#   ./script/perf/safari.py <url> <dir>

import re
import subprocess

import mac
from measure import SIZE, Cpu, Tail, measure
from webkit import WebKit

# What the Mac logs of a command buffer that its GPU aborted in Safari's GPU
# process: a hang, a timeout, a stall that held up the screen, or a victim of
# another's. `log stream`'s own first line names the predicate and is no such.
METAL = (
    'process BEGINSWITH "com.apple.WebKit.GPU" AND eventMessage CONTAINS "command buffer"'
)
ABORTED = re.compile(r"\(Metal\) .*command buffer")
# Why, as the Mac words it; a line worded otherwise is kept whole.
CAUSE = re.compile(r"during execution\. (.*) \([0-9a-f]{8}:")
# Safari's processes by role (measure.py `Cpu`), by the service each runs.
ROLES = {
    "com.apple.WebKit.WebContent": "page",
    "com.apple.WebKit.GPU": "gpu",
    "MTLCompilerService": "compiler",
}
SERVICE = re.compile(rf"/({'|'.join(map(re.escape, ROLES))})(?: |$)")


def role(_, cmd):
    m = SERVICE.search(cmd)
    return ROLES[m[1]] if m else None


class Safari(WebKit):
    """Safari started afresh, and quit as the run ends. The state is the
    Mac's, then how many command buffers the Mac logged as aborted in its GPU
    process since the last capture, and why, then the CPU its page, its GPU
    process and the Metal compiler took.

    Its window is `SIZE` in CSS pixels on whichever screen it opens, at that
    screen's device pixels a CSS pixel, and no smaller only where the screen
    is as wide. Safari takes no scale as Chrome does, and no screen is named
    for it: one that is not attached would end the run, and a Mac's built-in
    screen at two device pixels is narrower than `SIZE`. So one build draws
    2.0 Mpx on a screen of one device pixel and 7.9 on the built-in one, and
    the two are not one measurement: `drawn` in the run's values says which,
    and gate.py refuses to compare them."""

    @staticmethod
    def running():
        """safaridriver drives only a Safari it starts, and quitting the
        user's would close their windows, so a running Safari is refused."""
        on = subprocess.run(["pgrep", "-x", "Safari"], stdout=subprocess.DEVNULL)
        return "Safari is running" if on.returncode == 0 else None

    def __init__(self, url):
        self.cpu = Cpu(role)
        self.log = subprocess.Popen(
            ["/usr/bin/log", "stream", "--style", "compact", "--predicate", METAL],
            stdout=subprocess.PIPE,
            text=True,
            errors="replace",
        )
        self.metal = Tail(self.log.stdout, ABORTED)
        super().__init__(url, {"browserName": "safari"}, SIZE)

    def state(self):
        lines = self.metal.take()
        causes = [m[1] if (m := CAUSE.search(s)) else s[-120:] for s in lines]
        metal = f"metal_errors {len(causes)} {'; '.join(causes)}".rstrip()
        return f"{mac.state()}; {metal}; {self.cpu.take()}"

    def close(self):
        try:
            super().close()
        finally:
            self.log.terminate()
            self.log.wait()
            subprocess.run(["osascript", "-e", 'quit app "Safari"'])


# The run holds its device from before it starts (`measure.hold`).
mac.hold()
measure(Safari)
