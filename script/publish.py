#!/usr/bin/env python3
# Publishes to the bucket, and only there: the bucket's `source`, which
# clients read at boot, is the one record of which version of each kind is
# served, and each version holds its own manifest beside its tiles, its format
# and shards. A publish reads both out of the bucket, makes the declaration
# of them (pxl-geo-bake bin/declare_source.rs, refusing what a client would)
# and uploads it; nothing in the tree changes, so nothing is deployed, and
# the one thing built is that declaring tool.
#
# `upload <kind>` uploads `out/<kind>.publish/` as a new version, named
# `v<format>.<UTC day>.<n>`, n past every version of that format and day the
# bucket holds, so no version is written over, and serves nothing: what the
# clients deployed read stays as it is until a version is served. `<kind>`
# uploads and serves it. `serve <kind> <version>` serves a version the bucket
# holds, as a new format's is with the client that reads it, and going back
# is. `override <shard> <kind> <version>` serves that version for a shard
# and the tiles under it, `-` dropping it. Uses the AWS CLI's `r2` profile,
# its endpoint_url or `$R2` the bucket's S3 endpoint (publish-bucket).
#
#   ./script/publish.py <terrain|place|division|earth>…
#   ./script/publish.py upload <terrain|place|division|earth>…
#   ./script/publish.py serve <kind> <version>
#   ./script/publish.py override <shard> <kind> <version|->

import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import r2

REPO = Path(__file__).resolve().parent.parent
BUCKET = "pixelet-tiles"
KINDS = ["terrain", "place", "division", "earth"]
SOURCE = "source"
# Beside a copy's tiles, what it is: never uploaded.
MANIFEST = "manifest.json"
ROUTE = json.loads((REPO / "spec/spec/route.json").read_text())


class Refused(Exception):
    """Raised, so what is open is closed on the way out; said once at the top."""


def die(message):
    raise Refused(message)


def tile_object(version, name):
    """A tile's object under its kind, route.json's `object` filled in."""
    return ROUTE["roots"]["TILE_ROOT"]["object"].replace("{version}", version).replace("{name}", name)


def endpoint():
    return r2.profile_endpoint() or die("no endpoint for the `r2` profile: set its endpoint_url, or export $R2 (publish-bucket)")


def aws(args, inherit=False, none=False):
    # AWS CLI v2 sends checksum headers R2 refuses.
    env = {**os.environ, "AWS_REQUEST_CHECKSUM_CALCULATION": "when_required"}
    out = subprocess.run(["aws", *args, "--profile", "r2", "--endpoint-url", endpoint()], env=env, capture_output=not inherit)
    # `s3 ls` exits 1, silent, on a prefix that holds nothing.
    if none and out.returncode == 1 and not out.stderr:
        return ""
    if out.returncode != 0:
        die(f"aws {' '.join(args[:3])} failed" + ("" if inherit else f":\n{out.stderr.decode()}"))
    return "" if inherit else out.stdout.decode()


def files(directory):
    """The files under `directory`, as the sync uploads them."""
    return sum(1 for p in Path(directory).rglob("*") if p.is_file() and not p.name.endswith(".part") and p.name != MANIFEST)


def versions(kind):
    """The versions the bucket holds under `kind`."""
    listed = aws(["s3", "ls", f"s3://{BUCKET}/{kind}/"], none=True)
    return [m.group(1) for line in listed.splitlines() if (m := re.match(r"^PRE (.+)/$", line.strip()))]


def next_name(kind, fmt, day):
    """Past every `v<format>.<day>.<n>` the bucket holds under `kind`."""
    prefix = f"v{fmt}.{day}."
    taken = [int(name[len(prefix):]) for name in versions(kind) if name.startswith(prefix) and name[len(prefix):].isdigit()]
    return f"{prefix}{max([0, *taken]) + 1}"


def manifest_of(kind, version):
    """The manifest a version holds beside its tiles."""
    m = json.loads(aws(["s3", "cp", f"s3://{BUCKET}/{kind}/{tile_object(version, MANIFEST)}", "-"]))
    return {"version": version, "format": m["format"], **({"shard": m["shard"]} if m.get("shard") else {})}


def served_now():
    """What the bucket serves now, out of its `source` and the manifests of the versions it names."""
    declared_ = json.loads(aws(["s3", "cp", f"s3://{BUCKET}/{SOURCE}", "-"]))
    kind = {k: manifest_of(k, declared_["version"].get(k) or die(f"{SOURCE} serves no {k}")) for k in KINDS}
    override = {key: s["version"] for key, s in declared_["shard"].items() if s.get("version")}
    return {"kind": kind, "override": override}


def declared(record):
    """The declaration pxl-geo-bake writes of a record (bin/declare_source.rs), refusing what a client would."""
    out = subprocess.run(
        ["cargo", "run", "-q", "-p", "pxl-geo-bake", "--bin", "declare_source"],
        cwd=REPO / "rust",
        input=json.dumps(record).encode(),
        stdout=subprocess.PIPE,
    )
    return out.stdout if out.returncode == 0 else die("declare_source refused it")


def serve(declaration):
    """Revalidated on every boot, so a publish is seen at once."""
    with tempfile.NamedTemporaryFile(suffix=".json") as f:
        f.write(declaration)
        f.flush()
        aws(["s3", "cp", f.name, f"s3://{BUCKET}/{SOURCE}", "--content-type", "application/json", "--cache-control", "no-cache"], inherit=True)
    print(f"[publish] {SOURCE} uploaded")


def upload(record, kind):
    """`out/<kind>.publish/` uploaded as a new version, and what it is: served
    by no one until a declaration names it (`serve`)."""
    directory = REPO / "out" / f"{kind}.publish"
    if not directory.is_dir():
        die(f"{kind}: no {directory.relative_to(REPO)}: bake it (with `--stage publish` where it has stages)")
    # What the version is, fixed with it: the copy as its own manifest says.
    manifest = json.loads((directory / MANIFEST).read_text())
    fmt, shard = manifest.get("format"), manifest.get("shard")
    if not isinstance(fmt, int):
        die(f"{directory.relative_to(REPO)}/{MANIFEST} names no format the set is in")
    name = next_name(kind, fmt, datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d"))
    version = {"version": name, "format": fmt, **({"shard": shard} if shard else {})}
    # Refused before a byte goes up: a version no declaration can name serves no one.
    declared({**record, "kind": {**record["kind"], kind: version}})

    objects = files(directory)
    at = f"s3://{BUCKET}/{kind}/{tile_object(name, '')}"
    print(f"[publish/{kind}] {objects} objects to {kind}/{name}")
    # Packed already: the encoding goes with each object (publish-bucket "Upload").
    aws(
        [
            "s3", "sync", str(directory), at,
            "--exclude", "*.part", "--exclude", MANIFEST,
            "--content-type", manifest["content_type"],
            "--content-encoding", manifest["content_encoding"],
            "--cache-control", "public, max-age=31536000, immutable",
        ],
        inherit=True,
    )
    stored = len([line for line in aws(["s3", "ls", at, "--recursive"]).splitlines() if line.strip()])
    if stored != objects:
        die(f"{kind}/{name}: {stored} objects stored of {objects}; re-run the sync by hand before serving it")
    # Last, so a version with a manifest is one whole.
    aws(["s3", "cp", str(directory / MANIFEST), f"{at}{MANIFEST}", "--content-type", "application/json"], inherit=True)

    shutil.rmtree(directory)
    print(f"[publish/{kind}] {kind}/{name} uploaded")
    return version


USAGE = f"usage: ./script/publish.py [upload] <{'|'.join(KINDS)}>… | serve <kind> <version> | override <shard> <kind> <version|->"


def command(record, args):
    """What `args` changes of what the bucket serves, `record`: the
    declaration to serve, or none where nothing is served (`upload`)."""
    first, rest = (args[0], args[1:]) if args else ("", [])
    if first == "serve":
        if len(rest) != 2 or rest[0] not in KINDS:
            die("usage: ./script/publish.py serve <kind> <version>")
        kind, version = rest
        record["kind"][kind] = manifest_of(kind, version)
    elif first == "override":
        if len(rest) != 3 or rest[1] not in KINDS:
            die("usage: ./script/publish.py override <shard> <kind> <version|->")
        shard, kind, version = rest
        own = dict(record["override"].get(shard, {}))
        if version == "-":
            own.pop(kind, None)
        else:
            own[kind] = version
        if own:
            record["override"][shard] = own
        else:
            record["override"].pop(shard, None)
    elif first == "upload":
        if not rest or any(k not in KINDS for k in rest):
            die(f"usage: ./script/publish.py upload <{'|'.join(KINDS)}>…")
        for kind in rest:
            upload(record, kind)
        return None
    else:
        kinds = args
        if not kinds or any(k not in KINDS for k in kinds):
            die(USAGE)
        for kind in kinds:
            record["kind"][kind] = upload(record, kind)
    return record


def main():
    try:
        endpoint()
        served = command(served_now(), sys.argv[1:])
        if served:
            serve(declared(served))
    except Refused as e:
        print(f"[publish] {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
