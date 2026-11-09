#!/usr/bin/env python3
# Compares two builds by assembling a vendor's measurement: runs it on the base
# and the change by turns, keeping each run under tmp/perf/, and fails where
# the change's median runs past the base's: its load's longest frame by more
# than `LOAD_SLACK_MS`, its GPU's or its tick's p50 or p95 by more than
# `P50_SHARE` or `P95_SHARE`, its tick also by more than `TICK_SLACK_MS`. A dev
# build is never compared with a release build.
#
#   ./script/perf/gate.py <vendor> <base> <change> [rounds]
#
# A vendor is a script beside this one: chrome, safari, android-chrome,
# android-app, ios-safari or ios-app.

import json
import math
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = os.path.join(os.path.dirname(os.path.dirname(HERE)), "tmp", "perf")
# How much longer the change's first load may block the page than the base's.
LOAD_SLACK_MS = 3000
# How much more the change's GPU time may take than the base's.
P50_SHARE = 1.15
P95_SHARE = 1.3
# A tick's share is of little time: what it may run past that as well.
TICK_SLACK_MS = 0.5


def once(vendor, target, out):
    subprocess.run(
        [os.path.join(HERE, f"{vendor}.py"), target, out],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    with open(os.path.join(out, "values.json")) as f:
        return json.load(f)


def median(v):
    """A figure the window could not give (None) counts as none: NaN fails
    every test."""
    v = [math.nan if x is None else x for x in v]
    if any(math.isnan(x) for x in v):
        return math.nan
    return sorted(v)[(len(v) - 1) // 2]


def main():
    if len(sys.argv) < 4:
        print("gate.py <vendor> <base> <change> [rounds]", file=sys.stderr)
        return 2
    vendor, base, change = sys.argv[1:4]
    rounds = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    runs = {"base": [], "change": []}
    for i in range(rounds):
        for who, target in (("base", base), ("change", change)):
            out = os.path.join(RUNS, vendor, f"{who}-{i + 1}")
            runs[who].append(once(vendor, target, out))
    builds = {r["build"] for rs in runs.values() for r in rs}
    if len(builds) != 1:
        sys.exit(f"not compared: the runs' builds are {sorted(builds)}")
    print(f"{vendor}, {builds.pop()} build; runs in {os.path.join(RUNS, vendor)}")
    fails = []
    load = [median([r["load"] for r in runs[who]]) for who in ("base", "change")]
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
