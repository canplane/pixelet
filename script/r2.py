# The bucket, through the AWS CLI on R2's S3 endpoint: one object at a time
# (the build counter, the build log, the deploy's path tables, the source and
# a version's manifest), and the commands publish.py runs to list and sync a
# set. A key held to this bucket alone:
# CI's `R2_ACCESS_KEY_ID` and `R2_SECRET_ACCESS_KEY` handed in as
# AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, its endpoint from
# CLOUDFLARE_ACCOUNT_ID; or a machine's `r2` profile and its endpoint_url
# (publish-bucket).

import os
import re
import subprocess
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# The bucket the Worker is bound to (ts/server/edge/wrangler.jsonc, which
# binds one).
with open(os.path.join(REPO, "ts/server/edge/wrangler.jsonc")) as f:
    BUCKET = re.search(r'"bucket_name":\s*"([^"]+)"', f.read())[1]
# Where an object's bytes lie on their way, until the read or write ends: the
# checkout's tmp/ (AGENTS.md).
TMP = os.path.join(REPO, "tmp")


class Unreachable(Exception):
    """The bucket could not be reached, or an object read or written."""


def endpoint():
    """The bucket's S3 endpoint: `$R2`, else the account's, else the `r2` profile's."""
    if os.environ.get("R2"):
        return os.environ["R2"]
    if os.environ.get("CLOUDFLARE_ACCOUNT_ID"):
        return f"https://{os.environ['CLOUDFLARE_ACCOUNT_ID']}.r2.cloudflarestorage.com"
    got = subprocess.run(["aws", "configure", "get", "endpoint_url", "--profile", "r2"], capture_output=True, text=True)
    if not got.stdout.strip():
        raise Unreachable("no endpoint: set CLOUDFLARE_ACCOUNT_ID, $R2 or the `r2` profile's endpoint_url")
    return got.stdout.strip()


def aws(*args, capture=True):
    """The AWS CLI's command `args` run against the bucket's endpoint, its
    output captured or, without `capture`, the terminal's."""
    # A key in the environment is CI's; else the machine's `r2` profile.
    profile = [] if os.environ.get("AWS_ACCESS_KEY_ID") else ["--profile", "r2"]
    # AWS CLI v2 sends checksum headers R2 refuses.
    env = {**os.environ, "AWS_REQUEST_CHECKSUM_CALCULATION": "when_required", "AWS_DEFAULT_REGION": "auto"}
    return subprocess.run(["aws", *args, *profile, "--endpoint-url", endpoint()], env=env, capture_output=capture)


def get(key):
    """The object's bytes; None where there is no such object."""
    os.makedirs(TMP, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TMP) as d:
        out = os.path.join(d, "body")
        run = aws("s3api", "get-object", "--bucket", BUCKET, "--key", key, out)
        if run.returncode != 0:
            err = run.stderr.decode(errors="replace")
            if "NoSuchKey" in err or "404" in err:
                return None
            raise Unreachable(f"get {key}: {err.strip()}")
        with open(out, "rb") as f:
            return f.read()


def put(key, body, content_type, cache_control=None):
    """Writes the object whole; R2 has no append."""
    os.makedirs(TMP, exist_ok=True)
    with tempfile.NamedTemporaryFile(delete=False, dir=TMP) as f:
        f.write(body)
    cache = ["--cache-control", cache_control] if cache_control else []
    try:
        run = aws("s3api", "put-object", "--bucket", BUCKET, "--key", key, "--body", f.name, "--content-type", content_type, *cache)
    finally:
        os.unlink(f.name)
    if run.returncode != 0:
        raise Unreachable(f"put {key}: {run.stderr.decode(errors='replace').strip()}")
