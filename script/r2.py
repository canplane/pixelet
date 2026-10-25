# One object of the bucket at a time, read or written through wrangler,
# Cloudflare's CLI for it: the build counter, the build log and the deploy's
# path tables. wrangler runs as the edge's deploy runs it, from its package,
# so ts/deno.lock pins it, and takes its credential as it does there: CI's
# CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID, or a machine's own login.
# A whole set's upload is the AWS CLI's (publish.py), which syncs a directory.

import os
import subprocess
import tempfile
from pathlib import Path

BUCKET = "pixelet-tiles"
WRANGLER = ["wrangler"]


class Unreachable(Exception):
    """wrangler could not read or write the object."""


def get(key):
    """The object's bytes; None where there is no such object."""
    run = subprocess.run([*WRANGLER, "r2", "object", "get", f"{BUCKET}/{key}", "--remote", "--pipe"], cwd=EDGE, capture_output=True)
    if run.returncode != 0:
        err = run.stderr.decode(errors="replace")
        if "does not exist" in err or "NoSuchKey" in err or "404" in err:
            return None
        raise Unreachable(f"get {key}: {err.strip()}")
    return run.stdout


def put(key, body, content_type):
    """Writes the object whole; R2 has no append."""
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(body)
    try:
        run = subprocess.run(
            [*WRANGLER, "r2", "object", "put", f"{BUCKET}/{key}", "--remote", "--file", f.name, "--content-type", content_type],
            capture_output=True,
        )
    finally:
        os.unlink(f.name)
    if run.returncode != 0:
        raise Unreachable(f"put {key}: {run.stderr.decode(errors='replace').strip()}")
