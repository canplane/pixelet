#!/usr/bin/env python3
# One measurement of the app on an iPhone, installed over any earlier build, its
# data kept.
#
#   IOS_DEVICE=<devicectl id> ./script/perf/ios-app.py <app> <dir>

import os
import subprocess

from measure import measure, stream

APP = "me.plei.pixelet"


def ios_app(app):
    device = ["--device", os.environ["IOS_DEVICE"]]
    ctl = ["xcrun", "devicectl", "device"]
    subprocess.run(
        [*ctl, "install", "app", *device, app], check=True, stdout=subprocess.DEVNULL
    )
    yield from stream(
        [*ctl, "process", "launch", "--console", "--terminate-existing", *device, APP]
    )


measure(ios_app)
