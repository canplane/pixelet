#!/usr/bin/env python3
# What a deployment must answer, checked against a running one. Addresses
# come from spec/spec/route.json, which every relay is written from, so a
# moved root cannot leave this checking a dead URL. `--origin` skips the
# bucket's branches, which only the edge serves.
#
# A rough check, not a test: each answer's status, and what it plainly is —
# the app's HTML, the source's JSON, a tile's packed bytes once and not twice,
# the same bytes off the edge cache's hit, an empty page left empty, an
# asset's type. What depends on the edge's mood (whether the second ask hit
# the cache) is said, not failed.
#
#   ./script/smoke.py https://plei.me
#   ./script/smoke.py https://api.plei.me --origin

import json
import re
import secrets
import sys
import urllib.error
import urllib.request
import zlib
from pathlib import Path

SPEC = Path(__file__).resolve().parent.parent / "spec" / "spec"
route = json.loads((SPEC / "route.json").read_text())


def root(name):
    return route["roots"][name]["path"]


def branch(name):
    b = route["branches"][name]
    return f"{root(b['under'])}/{b['name']}"


GEO_ROOT, API_ROOT, TILE_ROOT = root("GEO_ROOT"), root("API_ROOT"), root("TILE_ROOT")
SEARCH_URL, GEO_INTERNAL, API_INTERNAL = branch("SEARCH_URL"), branch("GEO_INTERNAL"), branch("API_INTERNAL")


def tile_object(version, name):
    """A tile's object under its kind, route.json's `object` filled in."""
    return route["roots"]["TILE_ROOT"]["object"].replace("{version}", version).replace("{name}", name)


# Any shipped asset's bucket key: the first ingested.json lists.
ASSET_KEY = json.loads((SPEC / "asset" / "ingested.json").read_text())["files"][0]["key"]

# A land page in Bundang: the zoom-15 tile's name.
PAGE = "010332102130223"
# PAGE's divisions page at z5, its first five digits: one zoom under the
# pyramid's top (pxl-spec-core lib.rs `TOP_ZOOM`). Written out, since
# computing it would copy pxl-spec's arithmetic here.
DIVISION_TOP = "01033"
# A zoom-15 page in the Arctic Ocean, the pyramid's north-east corner: well
# shaped, and no bake holds it.
OPEN_SEA = "333333333333333"

# What every baked tile is packed in: zstd's frame magic, little-endian.
ZSTD_MAGIC = bytes([0x28, 0xB5, 0x2F, 0xFD])
# An XTP packet's magic, "XT": what a tile's bytes start with once unpacked.
XTP_MAGIC = b"XT"

# What a browser offers, but br, which the standard library cannot unpack:
# a transport encoding laid on a tile is taken off as a browser would, so
# what is checked is what the client is handed.
BROWSER_ENCODINGS = "gzip, deflate, zstd"


class Answer:
    def __init__(self, status, headers, body):
        self.status, self.headers, self.body = status, headers, body


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def nonce(url):
    """A unique query string, so a cached answer cannot pass for a dead deployment."""
    return f"{url}{'&' if '?' in url else '?'}_smoke={secrets.token_hex(8)}"


def ask(url, raw=False):
    """The answer, or why there was none. Raw is read as stored: the browser's
    encodings offered and the body left as it came; otherwise nothing is
    offered and the body comes plain. Only a tile wants raw."""
    req = urllib.request.Request(url, headers={"accept-encoding": BROWSER_ENCODINGS if raw else "identity"})
    try:
        with OPENER.open(req, timeout=30) as res:
            return Answer(res.status, res.headers, unpacked(res.headers, res.read()))
    except urllib.error.HTTPError as err:
        return Answer(err.code, err.headers, unpacked(err.headers, err.read()))
    except Exception as err:
        return type(err).__name__


def unpacked(headers, body):
    """The body as a browser hands it on: gzip or deflate taken off, zstd,
    which the client unpacks itself, left on."""
    enc = (headers.get("content-encoding") or "").lower()
    if enc == "gzip":
        return zlib.decompress(body, 16 + zlib.MAX_WBITS)
    if enc == "deflate":
        return zlib.decompress(body)
    return body


# What is wrong with an answer of the right status, or None.
def packed(a):
    """Packed once: the stored zstd frame, or the packet itself."""
    if a.body.startswith(ZSTD_MAGIC) or a.body.startswith(XTP_MAGIC):
        return None
    return f"neither a zstd frame nor a packet ({a.body[:4].hex()}): packed twice, or not a tile"


def typed(pattern):
    def look(a):
        t = a.headers.get("content-type", "")
        return None if re.search(pattern, t) else f"content-type {t or 'none'}"

    return look


def as_json(a):
    try:
        return json.loads(a.body)
    except ValueError:
        return None


def declared(a):
    s = as_json(a)
    if not isinstance(s, dict):
        return "not JSON"
    version = s.get("version") or {}
    missing = [k for k in ("terrain", "place", "division", "earth") if not isinstance(version.get(k), str)]
    return f"no version for {', '.join(missing)}" if missing else None


def listed(a):
    return None if isinstance(as_json(a), list) else "not a JSON list"


def empty(a):
    return None if not a.body else f"{len(a.body)}B of body"


def asset(a):
    wrong = typed(r"^(model/gltf-binary|image/(png|webp))")(a)
    if wrong:
        return wrong
    enc = a.headers.get("content-encoding")
    return f"content-encoding {enc}" if enc else None


def served_versions(base):
    """Each kind's version as the deployment's own `source` serves it."""
    a = ask(nonce(f"{base}{TILE_ROOT}/source"))
    if isinstance(a, str) or a.status != 200:
        return {}
    s = as_json(a)
    return (s or {}).get("version") or {}


def edge_checks(version):
    terrain, division = version.get("terrain", ""), version.get("division", "")
    return [
        ("/", 200, "the app itself, from the Worker's static assets", typed(r"^text/html"), False),
        (f"{TILE_ROOT}/source", 200, "the source's declaration, out of the bucket", declared, False),
        (f"{SEARCH_URL}?q=station", 200, "pxl-geo, the one geo query with no page to bake", listed, False),
        (f"{TILE_ROOT}/terrain/{tile_object(terrain, PAGE)}", 200, "a terrain page, out of the bucket", packed, True),
        (f"{TILE_ROOT}/division/{tile_object(division, DIVISION_TOP)}", 200, "the divisions' top page over PAGE", packed, True),
        (
            f"{TILE_ROOT}/terrain/{tile_object(terrain, OPEN_SEA)}",
            204,
            "a page no bake holds is answered empty — HTML here means `run_worker_first` is gone",
            empty,
            True,
        ),
        # Asked as a browser asks, so an encoding laid on it would show.
        (f"/{ASSET_KEY}", 200, "a content-addressed asset, its own root", asset, True),
        *common_checks(),
    ]


def common_checks():
    return [
        (f"{GEO_INTERNAL}/x", 401, "pxl-geo's internal prefix, gated by the admin token", None, False),
        (f"{API_INTERNAL}/x", 401, "pxl-api's internal prefix, gated the same way", None, False),
        (f"{API_ROOT}/nothing-is-here", 404, "pxl-api answers its root and has no route yet", None, False),
    ]


def origin_checks():
    return [
        (f"{SEARCH_URL}?q=station", 200, "pxl-geo, reached through Caddy without the edge", None, False),
        *common_checks(),
        (f"{GEO_ROOT}/nothing-is-here", 404, "pxl-geo answers its root and not this", None, False),
    ]


def line(mark, got, path, note):
    path = path[:55] + "…" if len(path) > 58 else path
    print(f"  {mark} {got:<5} {path:<58} {note}")


def main():
    args = [a for a in sys.argv[1:] if a != "--origin"]
    origin_only = "--origin" in sys.argv[1:]
    if not args:
        print("usage: ./script/smoke.py <base-url> [--origin]", file=sys.stderr)
        return 2
    base = args[0].rstrip("/")

    version = {} if origin_only else served_versions(base)
    checks = origin_checks() if origin_only else edge_checks(version)
    print(f"[smoke] {base}{' (origin, behind the edge)' if origin_only else ''} — {len(checks)} checks\n")

    failed = 0
    for path, want, why, look, raw in checks:
        a = ask(nonce(f"{base}{path}"), raw)
        if isinstance(a, str):
            wrong = a
        elif a.status != want:
            wrong = f"expected {want}"
        else:
            wrong = look(a) if look else None
        if wrong is not None:
            failed += 1
        got = "-" if isinstance(a, str) else str(a.status)
        line("ok  " if wrong is None else "FAIL", got, path, f"{len(a.body)}B" if wrong is None else f"{wrong} — {why}")

    # The edge cache's hit leaves by its own path (the Worker's
    # `CACHED_ENCODING`), so a first answer can be right and every hit after
    # it wrong: one tile asked twice at one URL must come back as the same bytes.
    if not origin_only:
        path = f"{TILE_ROOT}/terrain/{tile_object(version.get('terrain', ''), PAGE)}"
        url = nonce(f"{base}{path}")
        first, second = ask(url, True), ask(url, True)
        if isinstance(first, str) or isinstance(second, str):
            failed += 1
            line("FAIL", "-", f"{path} ×2", "a request failed")
        else:
            same = (
                first.status == second.status
                and first.body == second.body
                and first.headers.get("content-encoding") == second.headers.get("content-encoding")
            )
            wrong = "the second answer differs from the first: the cache's way out is broken" if not same else packed(second)
            if wrong is not None:
                failed += 1
            line("ok  " if wrong is None else "FAIL", str(second.status), f"{path} ×2", wrong or f"{len(second.body)}B twice")
            hit = second.headers.get("cf-cache-status")
            if wrong is None and hit != "HIT":
                print(f"  warn       the second ask was not a cache hit (cf-cache-status {hit or 'none'}): its way out went unchecked")

    print()
    if failed:
        print(f"[smoke] {failed} failed", file=sys.stderr)
        return 1
    print("[smoke] all answered as they should")
    return 0


if __name__ == "__main__":
    sys.exit(main())
