#!/usr/bin/env -S deno run -A
// What a deployment must answer, checked against a running one. Addresses
// come from the generated route constants, so a moved root cannot leave this
// checking a dead URL. `--origin` skips the bucket's branches, which only the
// edge serves.
//
//   deno run -A script/smoke.ts https://plei.me
//   deno run -A script/smoke.ts https://api.plei.me --origin

import type { u32 } from "@xpute/kit/abi/word.ts";
import { INGESTED } from "../apps/client/src/catalog/asset/ingested.spec.ts";

import { API_INTERNAL, API_ROOT, GEO_INTERNAL, GEO_ROOT, SEARCH_URL, TILE_ROOT, tileObject } from "../apps/client/src/config/route.spec.ts";

/** A land page in Bundang: the zoom-15 tile's name. */
const PAGE = "010332102130223";

/** PAGE's divisions page at the pyramid's top, z5: its first five digits,
 * the one a reader asks for first. Written out, since computing it would copy
 * pxl-spec's arithmetic into TypeScript. */
const DIVISION_TOP = "01033";

const ASSET_KEY = Object.values(INGESTED)[0].key;

interface Check {
  path: string;
  want: u32;
  why: string;
  /** Deno's fetch offers no zstd, and a stored object is served only in its own encoding. */
  encoding?: string;
}

const BROWSER_ENCODINGS = "gzip, deflate, br, zstd";

/** Each kind's version as the deployment's own `source` serves it. */
async function servedVersions(base: string): Promise<Record<string, string>> {
  try {
    const res = await fetch(`${base}${TILE_ROOT}/source?_smoke=${crypto.randomUUID()}`);
    return res.ok ? (await res.json()).version ?? {} : {};
  } catch {
    return {};
  }
}

function edgeChecks(version: Record<string, string>): Check[] {
  const terrain = `${TILE_ROOT}/terrain/${tileObject(version.terrain, PAGE)}`;
  return [
    { path: "/", want: 200, why: "the app itself, from the Worker's static assets" },
    { path: `${TILE_ROOT}/source`, want: 200, why: "the source's declaration, out of the bucket" },
    { path: `${SEARCH_URL}?q=${encodeURIComponent("강남")}`, want: 200, why: "pxl-geo, the one geo query with no page to bake" },
    { path: terrain, want: 200, why: "a terrain page, out of the bucket" },
    { path: `${TILE_ROOT}/division/${tileObject(version.division, DIVISION_TOP)}`, want: 200, why: "the divisions' top page over PAGE" },
    { path: `/${ASSET_KEY}`, want: 200, why: "a content-addressed asset, its own root" },
    ...commonChecks(),
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

const checks = originOnly ? originChecks() : edgeChecks(await servedVersions(base));
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
