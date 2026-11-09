# The phone adb reaches.

import os
import subprocess

ADB = os.environ.get(
    "ADB", os.path.expanduser("~/Library/Android/sdk/platform-tools/adb")
)


def adb(*args):
    return subprocess.run(
        [ADB, *args], check=True, stdout=subprocess.PIPE, text=True
    ).stdout
