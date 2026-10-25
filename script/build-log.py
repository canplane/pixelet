#!/usr/bin/env python3
# What each published build carried, and the number each one takes. The
# number is the official count, one object in the bucket, so no history
# rewrite renumbers it and it is never reset; the log records commit
# subjects, since main's history is rewritten and a sha may stop resolving.
# Each is read, added to and written back whole, since R2 has no append; the
# workflow's concurrency groups keep two takes, or two records, apart.
#
#   ./script/build-log.py                      every build, newest first
#   ./script/build-log.py --take               the next build number, taken (CI)
#   ./script/build-log.py --record 18 --web --server --smoke success     append one (CI)

import json
import signal
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# Its output piped into a pager or head ends quietly when the reader stops.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

import r2

ROOT = Path(__file__).resolve().parent.parent
KEY = "_meta/build-log.jsonl"
COUNTER = "_meta/build-counter"
CACHE = ROOT / "ts/client/.build_number"


def die(message: str):
    print(f"[build-log] {message}", file=sys.stderr)
    sys.exit(1)


def smoke_of(result: str | None) -> str | None:
    if result == "success":
        return "pass"
    if result == "failure":
        return "fail"
    return None


def git(*args: str) -> str | None:
    out = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    return out.stdout.strip() if out.returncode == 0 else None


def fetched(key: str) -> str:
    """The object's text. A missing one is refused: treating it as empty would
    overwrite the log, or restart the count."""
    try:
        got = r2.get(key)
    except r2.Unreachable as e:
        die(str(e))
    if got is None:
        die(f"no {key} in {r2.BUCKET}, and an empty one is not assumed; put one there by hand to start it")
    return got.decode()


def take() -> None:
    """The next build number, taken off the counter and printed."""
    current = fetched(COUNTER).strip()
    if not current.isdigit():
        die(f"the counter holds {json.dumps(current[:40])}, not a count; no number was taken")
    n = int(current) + 1
    try:
        r2.put(COUNTER, f"{n}\n".encode(), "text/plain")
    except r2.Unreachable as e:
        die(f"{e}; #{n} was not taken and the next build would reuse it")
    print(n)


def read() -> tuple[str, list[dict]]:
    raw = fetched(KEY)
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
    entry = line_for(n, sha, entries, web, server, smoke)
    body = raw if raw.endswith("\n") or raw == "" else raw + "\n"
    body += json.dumps(entry, separators=(",", ":"), ensure_ascii=False) + "\n"
    try:
        r2.put(KEY, body.encode(), "application/x-ndjson")
    except r2.Unreachable as e:
        die(f"{e}; #{n} went out and is not recorded")
    print(f"#{n} recorded: {len(entry['commits'])} commit(s) from {sha[:8]}")


def line_for(n: int, sha: str, entries: list[dict], web: bool, server: bool, smoke: str | None) -> dict:
    """This build's record, after the log's last."""
    # A previous sha that no longer resolves leaves only this build's tip.
    previous = entries[-1]["sha"] if entries else None
    reachable = previous is not None and git("cat-file", "-e", f"{previous}^{{commit}}") is not None
    subjects = git("log", "--format=%s", f"{previous}..HEAD" if reachable else "-1") or ""

    return {
        "n": n,
        "sha": sha,
        "at": datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "web": web,
        "server": server,
        "smoke": smoke,
        "from": previous if reachable else None,
        "commits": [s for s in subjects.split("\n") if s],
    }


def show(entry: dict) -> None:
    went = " + ".join(w for w, on in (("web", entry["web"]), ("server", entry["server"])) if on)
    smoke = "" if "smoke" not in entry else "  · smoke did not run" if entry["smoke"] is None else f"  · smoke {entry['smoke']}"
    print(f"#{entry['n']}  {entry['at'][:16].replace('T', ' ')}  {entry['sha'][:8]}  {went}{smoke}")
    for subject in entry["commits"]:
        print(f"    {subject}")


args = sys.argv[1:]
if args == ["--take"]:
    take()
elif args[:1] == ["--record"]:
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
    die(f"takes no argument but --take or --record: {json.dumps(args[0])}")
