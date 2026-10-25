#!/usr/bin/env python3
# Builds and publishes the client and its Worker under `PXL_BUILD_NUMBER`
# from CI. Without a number it publishes unnumbered (`#<last>-<id>`,
# ts/app/web/vite.config.ts `buildNumber`).
#
# The deployment runs across the trees, so it is the repository's: the
# page's bundle, built and staged beside the Worker (ts/server/edge
# script/stage.sh); the panics' path table, kept in the bucket (r2.py); the
# Worker, published by its own package (its `deploy` task, wrangler); and
# the site read back. Everything that can fail before the Worker goes out
# runs before it, so a failure says truly what was published, and the last
# step is the publish itself.
#
#   ./script/deploy.py

import os
import re
import secrets
import subprocess
import sys
import urllib.request
from pathlib import Path

import r2

REPO = Path(__file__).resolve().parent.parent
EDGE = REPO / "ts/server/edge"
NUMBER = os.environ.get("PXL_BUILD_NUMBER")
# The panics' `#n` → path table the page's build wrote beside its guest.
PATHS = REPO / "ts/app/web/src/guest.paths.json"
BUILT = EDGE / "dist/index.html"
# The custom domain the Worker answers on (wrangler.jsonc `routes`).
SITE = "https://plei.me"


def loads(html):
    """What a page loads: its hashed scripts and styles, which name the build."""
    return " ".join(sorted(re.findall(r'(?:src|href)="(/assets/[^"]+)"', html)))


def live():
    """Whether the site serves this build: what wrangler's exit cannot say once
    it failed after the Worker went out (a route, a domain)."""
    try:
        built = loads(BUILT.read_text())
        with urllib.request.urlopen(f"{SITE}/?_deploy={secrets.token_hex(8)}", timeout=30) as res:
            return res.status == 200 and built != "" and loads(res.read().decode()) == built
    except Exception:
        return False


def landed(yes):
    """Said to CI's next jobs (`published`), where the run has an output file."""
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a") as f:
            f.write(f"published={'true' if yes else 'false'}\n")


def step(name, args):
    print(f"[deploy] {name}", flush=True)
    code = subprocess.run(args, cwd=EDGE).returncode
    if code != 0:
        print(f"[deploy] {name} failed; nothing was published", file=sys.stderr)
        sys.exit(code)


print(f"[deploy] build #{NUMBER}" if NUMBER else "[deploy] no PXL_BUILD_NUMBER: publishing an unnumbered build", flush=True)

# Lists the files behind `-dirty`, which CI once reported and nothing local reproduced.
status = subprocess.run(["git", "status", "--porcelain"], cwd=EDGE, capture_output=True, text=True)
if status.returncode == 0 and status.stdout.strip():
    print(f"[deploy] the tree is not clean, so the build id will say -dirty:\n{status.stdout.strip()}", flush=True)

step("build", [str(EDGE / "script/stage.sh")])

# The `#n` → path table for this build's panics, kept beside the build log,
# never served. Before the publish: a table for a build that then failed to
# go out is harmless, a live build without one is not.
if NUMBER:
    print("[deploy] keep the paths", flush=True)
    try:
        r2.put(f"_meta/paths/{NUMBER}.json", PATHS.read_bytes(), "application/json")
    except r2.Unreachable as e:
        print(f"[deploy] keep the paths failed ({e}); nothing was published", file=sys.stderr)
        sys.exit(1)

print("[deploy] publish", flush=True)
published = subprocess.run(["deno", "task", "--cwd", str(EDGE), "deploy"]).returncode
if published != 0:
    # wrangler can fail after the Worker went out (a route, a domain), so what
    # landed is read off the site, not off its exit.
    yes = live()
    landed(yes)
    kept = f"; _meta/paths/{NUMBER}.json is kept" if NUMBER else ""
    print(f"[deploy] publish failed, and {SITE} serves {'this build all the same' if yes else 'an earlier one'}{kept}", file=sys.stderr)
    sys.exit(published)

landed(True)
print(f"[deploy] #{NUMBER} published" if NUMBER else "[deploy] published, unnumbered")
