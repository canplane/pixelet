#!/usr/bin/env -S deno run -A
// What a deployment must answer, checked against a running one.
//
//   deno run -A script/smoke.ts https://plei.me
//   deno run -A script/smoke.ts https://api.plei.me --origin
//
// The addresses come from the same generated constants the client and the
// Worker read (spec/pxl/spec/route.json), the versions from published.json and the
// asset key from the ingest's own table, so a version bump or a moved root
// cannot leave this checking a URL nobody serves any more. That is the whole
// reason it is a Deno script and not a list of curls: a hand-kept list of
// paths is exactly what the address space stopped being.
//
// `--origin` is the stack behind the edge — Caddy and the two servers, reached
// directly. The bucket's own branches are left out there, because the origin
// has never served them: a deployment answers a tile out of R2 at the edge,
// and the origin 404s it correctly.
//
// A check names what it expects and why, so a failure reads as a sentence
// rather than as a status code. The old addresses are checked too: they must
// be gone, and a 200 from one means a relay is still carrying the pre-09-18
// shape.

import type { u32 } from "@xpute/core/abi/word.ts";
import { PUBLISHED_VERSION } from "../apps/web/src/config/published.spec.ts";
import { INGESTED } from "../apps/web/src/catalog/asset/ingested.spec.ts";

// The generated constants directly: route.spec.ts imports nothing itself, and
// the root has no mapping into apps/web because nothing else here reaches in.
import { API_INTERNAL, API_ROOT, GEO_INTERNAL, GEO_ROOT, SEARCH_URL, TILE_ROOT } from "../apps/web/src/config/route.spec.ts";

/** A land page in Gangnam — the one infra/README's own procedure names, so a
 * check here and a check by hand are looking at the same object. */
const PAGE = "12720250262323200";

/** The reverse-geocode group PAGE falls in: its lanes four levels down
 * (pxl-spec bake/place_page.rs's `GROUP_SHAMT`), since a geocode object
 * covers 16 pages a side rather than one page. Written out rather than
 * computed, because computing it means the packed-lane arithmetic in
 * TypeScript that pxl-spec exists to be the only copy of. */
const GEOCODE_GROUP = "794946921889792";

/** Any content-addressed asset: the table's first entry, whatever it is. The
 * point is that the bucket answers the asset root, not which model does. */
const ASSET_KEY = Object.values(INGESTED)[0].key;

interface Check {
  path: string;
  want: u32;
  why: string;
  /** What this check offers to accept, where that is the point of it. The
   * default is a browser's list; Deno's own fetch offers no zstd, and a
   * deployment serves a stored object in its own encoding or not at all. */
  encoding?: string;
}

/** What every current browser sends, which is what the bucket branches are
 * checked as. */
const BROWSER_ENCODINGS = "gzip, deflate, br, zstd";

function edgeChecks(): Check[] {
  const tile = (kind: keyof typeof PUBLISHED_VERSION) => `${TILE_ROOT}/${kind}/${PUBLISHED_VERSION[kind]}/${PAGE}`;
  // A terrain page's key is within its level, and the level is the segment
  // before it; a leaf is level 0.
  const terrain = `${TILE_ROOT}/terrain/${PUBLISHED_VERSION.terrain}/0/${PAGE}`;
  return [
    { path: "/", want: 200, why: "the app itself, from the Worker's static assets" },
    { path: `${SEARCH_URL}?q=${encodeURIComponent("강남")}`, want: 200, why: "pxl-geo, the one geo query with no page to bake" },
    { path: terrain, want: 200, why: "a terrain page, out of the bucket" },
    { path: `${TILE_ROOT}/terrain/${PUBLISHED_VERSION.terrain}/_ocean`, want: 200, why: "the shared ocean page every water tile falls back to" },
    { path: tile("poi"), want: 200, why: "a POI page" },
    { path: `${TILE_ROOT}/geocode/${PUBLISHED_VERSION.geocode}/${GEOCODE_GROUP}`, want: 200, why: "a reverse-geocode group" },
    { path: `${TILE_ROOT}/geocode/${PUBLISHED_VERSION.geocode}/_dict`, want: 200, why: "the dictionary every group's numbers are read against" },
    { path: `/${ASSET_KEY}`, want: 200, why: "a content-addressed asset, its own root" },
    // The other half of the rule above: what makes the packed bytes the only
    // thing the edge ever holds is that there is no decoded copy to hold. A
    // 200 here is the edge negotiating on the deployment's behalf again —
    // 526,936 bytes where the packed page is 8,543 (cloudflare-worker/index.ts).
    { path: terrain, want: 406, why: "a client offering no zstd is refused rather than handed a decoded copy", encoding: "gzip, deflate, br" },
    ...commonChecks(),
    // The pre-09-18 shape. A 200 from any of these means a relay somewhere is
    // still claiming the old root.
    { path: `${API_ROOT}/tiles/terrain/${PUBLISHED_VERSION.terrain}/${PAGE}`, want: 404, why: "the old tile address, which must be gone" },
    { path: `${API_ROOT}/search?q=x`, want: 404, why: "the old search address, which must be gone" },
  ];
}

/** The gates and the fallbacks, which the edge and the origin answer alike —
 * both are in front of the same two servers. */
function commonChecks(): Check[] {
  return [
    { path: `${GEO_INTERNAL}/x`, want: 401, why: "pxl-geo's internal prefix, gated by the admin token" },
    { path: `${API_INTERNAL}/x`, want: 401, why: "pxl-api's internal prefix, gated the same way" },
    { path: `${API_ROOT}/nothing-is-here`, want: 404, why: "pxl-api answers its root and has no route yet" },
  ];
}

function originChecks(): Check[] {
  return [
    { path: `${SEARCH_URL}?q=${encodeURIComponent("강남")}`, want: 200, why: "pxl-geo, reached through Caddy without the edge" },
    ...commonChecks(),
    { path: `${GEO_ROOT}/nothing-is-here`, want: 404, why: "pxl-geo answers its root and not this" },
  ];
}

const argv = Deno.args.filter((a) => a !== "--origin");
const originOnly = Deno.args.includes("--origin");
const base = argv[0]?.replace(/\/$/, "");
if (!base) {
  console.error("usage: deno run -A script/smoke.ts <base-url> [--origin]");
  Deno.exit(2);
}

const checks = originOnly ? originChecks() : edgeChecks();
console.log(`[smoke] ${base}${originOnly ? " (origin, behind the edge)" : ""} — ${checks.length} checks\n`);

let failed = 0;
for (const { path, want, why, encoding } of checks) {
  // A query string of its own on every request: Cloudflare caches what it is
  // told to, and a check that read a cached answer would pass for a
  // deployment that is no longer there.
  const url = `${base}${path}${path.includes("?") ? "&" : "?"}_smoke=${crypto.randomUUID()}`;
  let got: u32 | string;
  let size = 0;
  try {
    const res = await fetch(url, { redirect: "manual", headers: { "accept-encoding": encoding ?? BROWSER_ENCODINGS } });
    got = res.status;
    size = (await res.arrayBuffer()).byteLength;
  } catch (err) {
    got = `${err instanceof Error ? err.name : "failed"}`;
  }

  const ok = got === want;
  if (!ok) failed++;
  const mark = ok ? "ok  " : "FAIL";
  console.log(`  ${mark} ${String(got).padEnd(5)} ${path.length > 58 ? path.slice(0, 55) + "…" : path.padEnd(58)} ${ok ? `${size}B` : `expected ${want} — ${why}`}`);
}

console.log();
if (failed > 0) {
  console.error(`[smoke] ${failed} of ${checks.length} failed`);
  Deno.exit(1);
}
console.log(`[smoke] all ${checks.length} answered as they should`);
