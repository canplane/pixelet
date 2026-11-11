#!/usr/bin/env python3
# The iPhone devicectl or safaridriver reaches. Run, it runs a command holding
# the iPhone (`hold`):
#
#   ./script/perf/ios.py <command...>

import measure


def state():
    """The iPhone's state as a line: how busy its GPU is, which nothing here
    reads."""
    return "gpu_util None"


def hold():
    """Waits until this process holds the iPhone (`measure.hold`)."""
    measure.hold("iphone")


if __name__ == "__main__":
    measure.holding(hold)
