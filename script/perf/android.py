#!/usr/bin/env python3
# The phone adb reaches. Run, it runs a command holding the phone (`hold`):
#
#   ./script/perf/android.py <command...>

import os
import re
import subprocess

import measure

ADB = os.environ.get(
    "ADB", os.path.expanduser("~/Library/Android/sdk/platform-tools/adb")
)
KGSL = "/sys/class/kgsl/kgsl-3d0"
# The temperature thermalservice reads now, by its sensor's name.
AP = re.compile(r"mValue=([\d.]+), mType=\d+, mName=AP\b")


def adb(*args):
    return subprocess.run(
        [ADB, *args], check=True, stdout=subprocess.PIPE, text=True
    ).stdout


def state():
    """The phone's state as a line: how busy its GPU is in %, and its GPU's
    clock in Hz, as kgsl reads them now, then its AP's temperature in °C, as
    thermalservice reads it now."""
    busy = adb("shell", "cat", f"{KGSL}/gpu_busy_percentage").split()[0]
    hz = adb("shell", "cat", f"{KGSL}/devfreq/cur_freq").strip()
    thermal = adb("shell", "dumpsys", "thermalservice")
    ap = AP.search(thermal.partition("Current temperatures from HAL:")[2])
    return f"gpu_util {busy}% gpu_hz {hz} ap {ap[1] if ap else None}"


def shot(path):
    """Writes a PNG of the phone's screen to `path`."""
    with open(path, "wb") as f:
        subprocess.run([ADB, "exec-out", "screencap", "-p"], check=True, stdout=f)


def hold():
    """Waits until this process holds the phone, by its serial
    (`measure.hold`)."""
    measure.hold(f"android-{adb('get-serialno').strip()}")


if __name__ == "__main__":
    measure.holding(hold)
