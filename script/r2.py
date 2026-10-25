# The bucket's objects over R2's S3 API, through the AWS CLI: the one client
# every script here reaches the bucket with.
#
# Credentials, the first that is there:
# - CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID, as CI holds them: R2's S3
#   keys are the token's, as Cloudflare documents them (r2/api/tokens) — the
#   key id the token's id, the secret the SHA-256 of its value — and the
#   endpoint the account's.
# - the AWS CLI's `r2` profile and `$R2`, the bucket's S3 endpoint, as a
#   machine that publishes holds them (publish-bucket).

import hashlib
import json
import os
import subprocess
import tempfile
import urllib.request

BUCKET = "pixelet-tiles"


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
    elif os.environ.get("R2"):
        args = ["--profile", "r2", "--endpoint-url", os.environ["R2"]]
    else:
        raise Unreachable("no credential for the bucket: CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID, or the `r2` profile with $R2")
    _resolved = (args, env)
    return _resolved


def get(key):
    """The object's bytes and its ETag; None where there is no such object."""
    args, env = _cli()
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
