#!/usr/bin/env python3
# One measurement of the app on the phone adb reaches, from a fresh install,
# which removes the app's data. A Debug build loads its JavaScript from Metro
# on this machine, on `METRO_PORT`.
#
#   ./script/perf/android-app.py <apk> <dir>

import os
import subprocess

from android import ADB, adb
from measure import measure, stream

APP = "me.plei.pixelet"
METRO_PORT = os.environ.get("METRO_PORT", "8081")
# The log tag XPUTE.say writes under.
TAG = "xpute"


def android_app(apk):
    # An app that is not there yet has nothing to uninstall.
    subprocess.run([ADB, "uninstall", APP], stdout=subprocess.DEVNULL)
    adb("install", apk)
    adb("logcat", "-c")
    adb("reverse", "tcp:8081", f"tcp:{METRO_PORT}")
    try:
        adb("shell", "am", "start", "-n", f"{APP}/.MainActivity")
        yield from stream([ADB, "logcat", "-v", "raw", "-s", f"{TAG}:I"])
    finally:
        adb("shell", "am", "force-stop", APP)
        adb("reverse", "--remove", "tcp:8081")


measure(android_app)
