#!/usr/bin/env -S deno run -A
// What a deployment must answer, checked against a running one. Addresses
// come from the generated route constants, so a moved root cannot leave this
// checking a dead URL. `--origin` skips the bucket's branches, which only the
// edge serves.
//
//   deno run -A script/smoke.ts https://plei.me
//   deno run -A script/smoke.ts https://api.plei.me --origin

import type { u32 } from "@xpute/core/abi/word.ts";
import { PUBLISHED_VERSION } from "../apps/web/src/config/published.spec.ts";
import { INGESTED } from "../apps/web/src/catalog/asset/ingested.spec.ts";

import { API_INTERNAL, API_ROOT, GEO_INTERNAL, GEO_ROOT, SEARCH_URL, TILE_ROOT } from "../apps/web/src/config/route.spec.ts";

/** A land page in Bundang, the one infra/README's procedure names: the zoom-15 quadkey. */
const PAGE = "010332102130223";

/** PAGE's geocode group, four zooms coarser: its first eleven digits. Written
 * out, since computing it would copy pxl-spec's arithmetic into TypeScript. */
const GEOCODE_GROUP = "01033210213";

const ASSET_KEY = Object.values(INGESTED)[0].key;

interface Check {
  path: string;
  want: u32;
  why: string;
  /** Deno's fetch offers no zstd, and a stored object is served only in its own encoding. */
  encoding?: string;
}

const BROWSER_ENCODINGS = "gzip, deflate, br, zstd";

function edgeChecks(): Check[] {
  const tile = (kind: keyof typeof PUBLISHED_VERSION) => `${TILE_ROOT}/${kind}/${PUBLISHED_VERSION[kind]}/${PAGE}`;
  const terrain = tile("terrain");
  return [
    { path: "/", want: 200, why: "the app itself, from the Worker's static assets" },
    { path: `${SEARCH_URL}?q=${encodeURIComponent("강남")}`, want: 200, why: "pxl-geo, the one geo query with no page to bake" },
    { path: terrain, want: 200, why: "a terrain page, out of the bucket" },
    { path: `${TILE_ROOT}/geocode/${PUBLISHED_VERSION.geocode}/${GEOCODE_GROUP}`, want: 200, why: "a reverse-geocode group" },
    { path: `${TILE_ROOT}/geocode/${PUBLISHED_VERSION.geocode}/_dict`, want: 200, why: "the dictionary every group's numbers are read against" },
    { path: `/${ASSET_KEY}`, want: 200, why: "a content-addressed asset, its own root" },
    ...commonChecks(),
    { path: `${API_ROOT}/tiles/terrain/${PUBLISHED_VERSION.terrain}/${PAGE}`, want: 404, why: "the old tile address, which must be gone" },
    { path: `${TILE_ROOT}/terrain/${PUBLISHED_VERSION.terrain}/0/12720250262323200`, want: 404, why: "the old page name, which must be gone" },
    { path: `${GEO_ROOT}/tiles/terrain/${PUBLISHED_VERSION.terrain}/${PAGE}`, want: 404, why: "the old tile address under the geo root, which must be gone" },
    { path: `${API_ROOT}/search?q=x`, want: 404, why: "the old search address, which must be gone" },
  ];
}

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
  // A unique query string, so a cached answer cannot pass for a dead deployment.
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
