#!/usr/bin/env python3
# The Mac this runs on. Run, it runs a command holding its GPU (`hold`):
#
#   ./script/perf/mac.py <command...>

import re
import subprocess

import measure

# How busy the GPU is, in %, as its IOAccelerator reports it.
BUSY = re.compile(r'"Device Utilization %"=(\d+)')


def state():
    """The Mac's state as a line: how busy its GPU is now, whoever keeps it
    busy, in %."""
    accel = subprocess.run(
        ["ioreg", "-r", "-d", "1", "-c", "IOAccelerator"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout
    busy = BUSY.search(accel)
    return f"gpu_util {busy[1] + '%' if busy else None}"


def hold():
    """Waits until this process holds the Mac's GPU, as every Rust test that
    opens it does (`measure.hold`)."""
    measure.hold("gpu")


if __name__ == "__main__":
    measure.holding(hold)
