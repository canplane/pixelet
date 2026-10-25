#!/usr/bin/env python3
# What each published build carried. Records commit subjects, since main's
# history is rewritten and a sha may stop resolving. Kept in R2 beside the
# counter; R2 has no append, so the workflow's `build-log` concurrency group
# keeps two writes from losing a line. wrangler, the bucket's CLI, runs as the
# edge's deploy runs it, from its package, so ts/deno.lock pins it.
#
#   ./script/build-log.py                      every build, newest first
#   ./script/build-log.py --record 18 --web --server --smoke success     append one (CI)

import json
import signal
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

# Its output piped into a pager or head ends quietly when the reader stops.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

ROOT = Path(__file__).resolve().parent.parent
BUCKET = "pixelet-tiles"
KEY = "_meta/build-log.jsonl"
CACHE = ROOT / "ts/client/.build_number"
EDGE = ROOT / "ts/server/edge"
WRANGLER = ["deno", "run", "-A", "--node-modules-dir=auto", "npm:wrangler"]


def die(message: str):
    print(f"[build-log] {message}", file=sys.stderr)
    sys.exit(1)


def smoke_of(result: str | None) -> str | None:
    if result == "success":
        return "pass"
    if result == "failure":
        return "fail"
    return None


def wrangler(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([*WRANGLER, *args], cwd=EDGE, capture_output=True, text=True)


def git(*args: str) -> str | None:
    out = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    return out.stdout.strip() if out.returncode == 0 else None


def read() -> tuple[str, list[dict]]:
    """A missing object is refused: treating it as empty would overwrite the log."""
    got = wrangler("r2", "object", "get", f"{BUCKET}/{KEY}", "--remote", "--pipe")
    if got.returncode != 0:
        print(got.stderr, file=sys.stderr)
        die(
            f"no log at {KEY}, and an empty one is not assumed. Start it with:\n"
            f"  printf '' | (cd ts/server/edge && {' '.join(WRANGLER)} r2 object put {BUCKET}/{KEY} --remote --pipe --content-type application/x-ndjson)"
        )
    raw = got.stdout
    entries = []
    for line in raw.split("\n"):
        if not line.strip():
            continue
        try:
            entries.append(json.loads(line))
        except json.JSONDecodeError:
            die(f"{KEY} holds a line that is not a record: {json.dumps(line[:60])}")
    # Refreshes the gitignored cache of the last published number.
    if entries:
        CACHE.write_text(f"{entries[-1]['n']}\n")
    return raw, entries


def record(n: int, web: bool, server: bool, smoke: str | None) -> None:
    sha = git("rev-parse", "HEAD") or die("not a git checkout; nothing to record")
    raw, entries = read()
    # A re-run finds its own record already there.
    if any(e["n"] == n for e in entries):
        print(f"#{n} is already recorded")
        return

    # A previous sha that no longer resolves leaves only this build's tip.
    previous = entries[-1]["sha"] if entries else None
    reachable = previous is not None and git("cat-file", "-e", f"{previous}^{{commit}}") is not None
    subjects = git("log", "--format=%s", f"{previous}..HEAD" if reachable else "-1") or ""

    entry = {
        "n": n,
        "sha": sha,
        "at": datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "web": web,
        "server": server,
        "smoke": smoke,
        "from": previous if reachable else None,
        "commits": [s for s in subjects.split("\n") if s],
    }

    body = raw if raw.endswith("\n") or raw == "" else raw + "\n"
    with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False) as f:
        f.write(body + json.dumps(entry, separators=(",", ":"), ensure_ascii=False) + "\n")
    put = wrangler("r2", "object", "put", f"{BUCKET}/{KEY}", "--remote", "--file", f.name, "--content-type", "application/x-ndjson")
    os.unlink(f.name)
    if put.returncode != 0:
        print(put.stderr, file=sys.stderr)
        die(f"could not write the log back; #{n} went out and is not recorded")
    print(f"#{n} recorded: {len(entry['commits'])} commit(s) from {sha[:8]}")


def show(entry: dict) -> None:
    went = " + ".join(w for w, on in (("web", entry["web"]), ("server", entry["server"])) if on)
    smoke = "" if "smoke" not in entry else "  · smoke did not run" if entry["smoke"] is None else f"  · smoke {entry['smoke']}"
    print(f"#{entry['n']}  {entry['at'][:16].replace('T', ' ')}  {entry['sha'][:8]}  {went}{smoke}")
    for subject in entry["commits"]:
        print(f"    {subject}")


args = sys.argv[1:]
if args[:1] == ["--record"]:
    try:
        n = int(args[1])
    except (IndexError, ValueError):
        n = -1
    if n < 0:
        die(f"--record wants a build number, not {json.dumps(args[1] if len(args) > 1 else None)}")
    at = args.index("--smoke") if "--smoke" in args else -1
    record(n, "--web" in args, "--server" in args, smoke_of(args[at + 1] if 0 <= at < len(args) - 1 else None))
elif not args:
    _, entries = read()
    for entry in reversed(entries):
        show(entry)
else:
    die(f"takes no argument but --record: {json.dumps(args[0])}")
