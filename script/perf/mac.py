# The Mac this runs on.

import re
import subprocess

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
