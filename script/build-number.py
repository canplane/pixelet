#!/usr/bin/env python3
# The number of the build the deployment serves, as it says itself at
# /build.json, cached in ts/client/.build_number: a build CI did not number is
# labeled `#<n>-<id>` from it (ts/build/stamp.ts `stamp`). Plain
# HTTPS, so it needs no credential; where the site cannot be reached, the
# cache stays as it was.
#
#   ./script/build-number.py

import json
import sys
import urllib.request
from pathlib import Path

CACHE = Path(__file__).resolve().parent.parent / "ts/client/.build_number"

try:
    import json5

    SITE = json5.loads((Path(__file__).resolve().parent.parent / "spec/spec/route.json5").read_text())["origins"]["SITE"]["url"]["$value"]
    with urllib.request.urlopen(f"{SITE}/build.json", timeout=5) as res:
        number = json.load(res).get("number")
except Exception as e:
    print(f"[build-number] not synced ({type(e).__name__}): this build is labeled from the cached number", file=sys.stderr)
    sys.exit(0)
if isinstance(number, int):
    CACHE.write_text(f"{number}\n")
    print(f"[build-number] #{number}")
