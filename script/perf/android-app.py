#!/usr/bin/env python3
# One measurement of the app on the phone adb reaches, from a fresh install,
# which removes the app's data. A Debug build loads its JavaScript from Metro
# on this machine, on `METRO_PORT`.
#
#   ./script/perf/android-app.py <apk> <dir>

import os
import subprocess

import android
from android import ADB, adb
from measure import measure, stream

APP = "me.plei.pixelet"
METRO_PORT = os.environ.get("METRO_PORT", "8081")
# The log tag @xpute/native-runtime `log` writes under.
TAG = "xpute"


class App:
    """The app from `apk`, launched afresh: a reach (measure.py). The screen
    and the state are the phone's."""

    @staticmethod
    def running():
        """Installing afresh ends any copy that ran, so nothing running is
        shared."""
        return None

    def __init__(self, apk):
        # An app that is not there yet has nothing to uninstall.
        subprocess.run([ADB, "uninstall", APP], stdout=subprocess.DEVNULL)
        adb("install", apk)
        adb("logcat", "-c")
        adb("reverse", "tcp:8081", f"tcp:{METRO_PORT}")
        self.lines = stream([ADB, "logcat", "-v", "raw", "-s", f"{TAG}:I"])
        try:
            adb("shell", "am", "start", "-n", f"{APP}/.MainActivity")
        except BaseException:
            self.close()
            raise

    def shot(self, path):
        android.shot(path)

    def state(self):
        return android.state()

    def close(self):
        self.lines.close()
        adb("shell", "am", "force-stop", APP)
        adb("reverse", "--remove", "tcp:8081")


# The run holds its device from before it starts (`measure.hold`).
android.hold()
measure(App)
