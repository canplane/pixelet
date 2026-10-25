#!/usr/bin/env python3
# What the deployment has used against its free allowances: R2's writes and
# Hetzner traffic, which the free Workers plan does not bound. Uses
# wrangler's credential and hcloud's active context; wrangler runs from the
# edge's package, so ts/deno.lock pins it.
#
#   ./script/usage.py

import json
import os
import signal
import subprocess
import sys
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

# Its output piped into a pager or head ends quietly when the reader stops.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

WORKER = "pixelet"
BUCKET = "pixelet-tiles"
SERVER = "pixelet-prod"
EDGE = Path(__file__).resolve().parent.parent / "ts/server/edge"
WRANGLER = ["deno", "run", "-A", "--node-modules-dir=auto", "npm:wrangler"]

WORKER_REQUESTS_PER_DAY = 100_000
R2_STORAGE_BYTES = 10e9
R2_CLASS_A_PER_MONTH = 1_000_000
R2_CLASS_B_PER_MONTH = 10_000_000

# An action in neither list is counted as A, the dearer class, and said aloud.
CLASS_A = {
    "ListBuckets",
    "PutBucket",
    "ListObjects",
    "PutObject",
    "CopyObject",
    "CompleteMultipartUpload",
    "CreateMultipartUpload",
    "LifecycleStorageTierTransition",
    "ListMultipartUploads",
    "UploadPart",
    "UploadPartCopy",
    "ListParts",
    "PutBucketEncryption",
    "PutBucketCors",
    "PutBucketLifecycleConfiguration",
}
CLASS_B = {
    "HeadBucket",
    "HeadObject",
    "GetObject",
    "UsageSummary",
    "GetBucketEncryption",
    "GetBucketLocation",
    "GetBucketCors",
    "GetBucketLifecycleConfiguration",
}
FREE = {"DeleteObject", "DeleteObjects", "DeleteBucket", "AbortMultipartUpload"}


def die(message: str):
    print(f"[usage] {message}", file=sys.stderr)
    sys.exit(1)


def run(cmd: list[str], cwd: Path | None = None) -> str:
    out = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if out.returncode != 0:
        die(f"{' '.join(cmd[:4])} failed:\n{out.stderr}")
    return out.stdout


def token() -> str:
    if set_ := os.environ.get("CLOUDFLARE_API_TOKEN"):
        return set_
    out = run([*WRANGLER, "auth", "token", "--json"], cwd=EDGE)
    return json.loads(out[out.index("{") :]).get("token") or die("wrangler has no credential: run `wrangler login`")


def cloudflare(auth: str, path: str, body: dict | None = None) -> dict:
    request = urllib.request.Request(
        f"https://api.cloudflare.com/client/v4{path}",
        data=None if body is None else json.dumps(body).encode(),
        headers={"authorization": f"Bearer {auth}", "content-type": "application/json"},
        method="GET" if body is None else "POST",
    )
    try:
        with urllib.request.urlopen(request) as res:
            got = json.load(res)
    except urllib.error.HTTPError as e:
        die(f"{path}: {e.read().decode()[:400]}")
    if got.get("errors"):
        die(f"{path}: {json.dumps(got['errors'])[:400]}")
    return got


def account(auth: str) -> str:
    if set_ := os.environ.get("CLOUDFLARE_ACCOUNT_ID"):
        return set_
    got = cloudflare(auth, "/accounts")["result"]
    if len(got) != 1:
        die(f"the credential reaches {len(got)} accounts; name one in CLOUDFLARE_ACCOUNT_ID")
    return got[0]["id"]


QUERY = """query($account: String!, $worker: String!, $bucket: String!, $monthStart: Date!, $weekStart: Date!, $today: Date!) {
  viewer { accounts(filter: { accountTag: $account }) {
    workersInvocationsAdaptive(limit: 10, filter: { scriptName: $worker, date_geq: $weekStart, date_leq: $today }, orderBy: [date_ASC]) {
      sum { requests errors } dimensions { date }
    }
    r2OperationsAdaptiveGroups(limit: 100, filter: { bucketName: $bucket, date_geq: $monthStart, date_leq: $today }) {
      sum { requests } dimensions { actionType }
    }
    r2StorageAdaptiveGroups(limit: 1, filter: { bucketName: $bucket, date_geq: $monthStart, date_leq: $today }, orderBy: [date_DESC]) {
      max { objectCount payloadSize metadataSize } dimensions { date }
    }
  } }
}"""


def count(n: float) -> str:
    return f"{round(n):,}"


def share(n: float, of: float) -> str:
    return f"{100 * n / of:.1f}%"


def gb(n: float) -> str:
    return f"{n / 1e9:.2f} GB"


def cloudflare_usage() -> None:
    auth = token()
    account_id = account(auth)
    now = datetime.now(timezone.utc)
    today = now.date().isoformat()
    week_start = (now - timedelta(days=6)).date().isoformat()
    month_start = today[:8] + "01"
    got = cloudflare(
        auth,
        "/graphql",
        {
            "query": QUERY,
            "variables": {"account": account_id, "worker": WORKER, "bucket": BUCKET, "monthStart": month_start, "weekStart": week_start, "today": today},
        },
    )
    accounts = got["data"]["viewer"]["accounts"]
    a = accounts[0] if accounts else die("the analytics returned no account")

    print(f"Worker '{WORKER}' — requests per UTC day, of {count(WORKER_REQUESTS_PER_DAY)} free")
    for r in a["workersInvocationsAdaptive"]:
        errors = f", {count(r['sum']['errors'])} errors" if r["sum"]["errors"] > 0 else ""
        print(f"  {r['dimensions']['date']}  {count(r['sum']['requests']):>9}  {share(r['sum']['requests'], WORKER_REQUESTS_PER_DAY):>6}{errors}")

    class_a = class_b = free = 0
    unknown = []
    for r in a["r2OperationsAdaptiveGroups"]:
        action = r["dimensions"]["actionType"]
        if action in CLASS_B:
            class_b += r["sum"]["requests"]
        elif action in FREE:
            free += r["sum"]["requests"]
        else:
            class_a += r["sum"]["requests"]
            if action not in CLASS_A:
                unknown.append(action)
    print(f"\nR2 '{BUCKET}' — this month since {month_start}")
    print(f"  Class A  {count(class_a):>11}  {share(class_a, R2_CLASS_A_PER_MONTH):>6} of {count(R2_CLASS_A_PER_MONTH)}")
    print(f"  Class B  {count(class_b):>11}  {share(class_b, R2_CLASS_B_PER_MONTH):>6} of {count(R2_CLASS_B_PER_MONTH)}")
    print(f"  free     {count(free):>11}")
    if unknown:
        print(f"  (counted as Class A, unlisted: {', '.join(unknown)})")
    stored = a["r2StorageAdaptiveGroups"][0]["max"] if a["r2StorageAdaptiveGroups"] else None
    if stored:
        size = stored["payloadSize"] + stored["metadataSize"]
        print(f"  stored   {gb(size):>11}  {share(size, R2_STORAGE_BYTES):>6} of {gb(R2_STORAGE_BYTES)}, {count(stored['objectCount'])} objects")


def hetzner_usage() -> None:
    s = json.loads(run(["hcloud", "server", "describe", SERVER, "-o", "json"]))
    out = s.get("outgoing_traffic") or 0

    def gib(n: float) -> str:
        return f"{n / 2**30:.2f} GiB"

    print(f"\nHetzner '{SERVER}' ({s['server_type']['name']}) — this billing period")
    print(f"  outgoing  {gib(out):>11}  {share(out, s['included_traffic']):>6} of {gib(s['included_traffic'])}")
    print(f"  incoming  {gib(s.get('ingoing_traffic') or 0):>11}")


cloudflare_usage()
hetzner_usage()
