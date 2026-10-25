# The bucket's objects over R2's S3 API, through the AWS CLI: the one client
# every script here reaches the bucket with.
#
# Credentials, the first that is there:
# - CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID, as CI holds them: R2's S3
#   keys are the token's, as Cloudflare documents them (r2/api/tokens) — the
#   key id the token's id, the secret the SHA-256 of its value — and the
#   endpoint the account's.
# - the AWS CLI's `r2` profile, with the bucket's S3 endpoint as its
#   `endpoint_url` or in `$R2`, as a machine that publishes holds them
#   (publish-bucket).
# - for a read alone, wrangler's own login, as any machine that runs the
#   client has it: a dev build reads the build log with nothing set up for the
#   bucket, as it always has. Writes are CI's and a publisher's, and want one
#   of the two above.

import hashlib
import json
import os
import subprocess
import tempfile
import urllib.request
from pathlib import Path

BUCKET = "pixelet-tiles"
# wrangler as the edge's deploy runs it, from its package, so ts/deno.lock pins it.
EDGE = Path(__file__).resolve().parent.parent / "ts/server/edge"
WRANGLER = ["deno", "run", "-A", "--node-modules-dir=auto", "npm:wrangler"]


class Unreachable(Exception):
    """No credential reaches the bucket, or the CLI refused."""


def _token_id(token, account):
    """The API token's id, which Cloudflare's verify answers with."""
    for path in ("/user/tokens/verify", f"/accounts/{account}/tokens/verify"):
        req = urllib.request.Request(f"https://api.cloudflare.com/client/v4{path}", headers={"authorization": f"Bearer {token}"})
        try:
            with urllib.request.urlopen(req, timeout=30) as res:
                got = json.load(res)
            if got.get("success") and got.get("result", {}).get("id"):
                return got["result"]["id"]
        except Exception:
            continue
    raise Unreachable("CLOUDFLARE_API_TOKEN did not verify as a user or an account token")


def profile_endpoint():
    """The `r2` profile's endpoint: `$R2` where it is set, else the profile's
    own `endpoint_url`; none where neither is."""
    if os.environ.get("R2"):
        return os.environ["R2"]
    got = subprocess.run(["aws", "configure", "get", "endpoint_url", "--profile", "r2"], capture_output=True, text=True)
    return got.stdout.strip() or None


_resolved = None


def _cli():
    """The AWS CLI's arguments and environment for the bucket."""
    global _resolved
    if _resolved:
        return _resolved
    env = {**os.environ, "AWS_DEFAULT_REGION": "auto", "AWS_REQUEST_CHECKSUM_CALCULATION": "when_required"}
    token, account = os.environ.get("CLOUDFLARE_API_TOKEN"), os.environ.get("CLOUDFLARE_ACCOUNT_ID")
    if token and account:
        env["AWS_ACCESS_KEY_ID"] = _token_id(token, account)
        env["AWS_SECRET_ACCESS_KEY"] = hashlib.sha256(token.encode()).hexdigest()
        env.pop("AWS_PROFILE", None)
        args = ["--endpoint-url", f"https://{account}.r2.cloudflarestorage.com"]
    elif endpoint := profile_endpoint():
        args = ["--profile", "r2", "--endpoint-url", endpoint]
    else:
        raise Unreachable("no credential for the bucket: CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID, or the `r2` profile with its endpoint_url (or $R2)")
    _resolved = (args, env)
    return _resolved


def _wrangler_get(key):
    """The object's bytes through wrangler's login, with no ETag to write over;
    None where there is no such object."""
    run = subprocess.run([*WRANGLER, "r2", "object", "get", f"{BUCKET}/{key}", "--remote", "--pipe"], cwd=EDGE, capture_output=True)
    if run.returncode != 0:
        err = run.stderr.decode(errors="replace")
        if "does not exist" in err or "NoSuchKey" in err or "404" in err:
            return None
        raise Unreachable(f"get {key} through wrangler: {err.strip()}")
    return run.stdout, None


def get(key):
    """The object's bytes and its ETag; None where there is no such object.
    With no credential for the bucket, read through wrangler's login, the ETag
    then None."""
    try:
        args, env = _cli()
    except Unreachable:
        return _wrangler_get(key)
    with tempfile.TemporaryDirectory() as d:
        out = os.path.join(d, "body")
        run = subprocess.run(["aws", "s3api", "get-object", "--bucket", BUCKET, "--key", key, out, *args], env=env, capture_output=True, text=True)
        if run.returncode != 0:
            if "NoSuchKey" in run.stderr:
                return None
            raise Unreachable(f"get {key}: {run.stderr.strip()}")
        with open(out, "rb") as f:
            return f.read(), json.loads(run.stdout)["ETag"]


def put(key, body, content_type, if_match=None):
    """Writes the object; with `if_match`, only over the version of that ETag,
    the condition judged in the same write. False where another write came
    between, True where this one landed."""
    args, env = _cli()
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(body)
    try:
        cond = ["--if-match", if_match] if if_match else []
        run = subprocess.run(
            ["aws", "s3api", "put-object", "--bucket", BUCKET, "--key", key, "--body", f.name, "--content-type", content_type, *cond, *args],
            env=env,
            capture_output=True,
            text=True,
        )
    finally:
        os.unlink(f.name)
    if run.returncode == 0:
        return True
    if if_match and ("PreconditionFailed" in run.stderr or "412" in run.stderr):
        return False
    raise Unreachable(f"put {key}: {run.stderr.strip()}")
