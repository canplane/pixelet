#!/usr/bin/env python3
# One measurement in the Mac's Safari.
#
#   ./script/perf/safari.py <url> <dir>

import subprocess

from measure import SIZE, measure
from webkit import webkit


def safari(url):
    running = subprocess.run(["pgrep", "-x", "Safari"], stdout=subprocess.DEVNULL)
    try:
        yield from webkit(url, {"browserName": "safari"}, SIZE)
    finally:
        # A session's end leaves the app open: what this run opened, it quits.
        if running.returncode != 0:
            subprocess.run(["osascript", "-e", 'quit app "Safari"'])


measure(safari)
