#!/usr/bin/env python3
# One measurement of the app on an iPhone, installed over any earlier build, its
# data kept.
#
#   IOS_DEVICE=<devicectl id> ./script/perf/ios-app.py <app> <dir>

import os
import subprocess

import ios
from measure import measure, stream

APP = "me.plei.pixelet"
CTL = ["xcrun", "devicectl", "device"]


class App:
    """The app at `app`, launched with its console: a reach (measure.py). The
    screen is devicectl's screenshot; the state is the iPhone's."""

    @staticmethod
    def running():
        """The launch ends any copy that runs (`--terminate-existing`), so
        nothing running is shared."""
        return None

    def __init__(self, app):
        self.device = ["--device", os.environ["IOS_DEVICE"]]
        subprocess.run(
            [*CTL, "install", "app", *self.device, app],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        self.lines = stream(
            [*CTL, "process", "launch", "--console", "--terminate-existing", *self.device, APP]
        )

    def shot(self, path):
        subprocess.run(
            [*CTL, "capture", "screenshot", *self.device, "--destination", path],
            check=True,
            stdout=subprocess.DEVNULL,
        )

    def state(self):
        return ios.state()

    def close(self):
        self.lines.close()


# The run holds its device from before it starts (`measure.hold`).
ios.hold()
measure(App)
