#!/usr/bin/env -S deno run -A
// What a deployment must answer, checked against a running one. Addresses
// come from spec/spec/route.json, which every relay is written from, so a
// moved root cannot leave this checking a dead URL. `--origin` skips the bucket's branches, which only the
// edge serves.
//
// A rough check, not a test: each answer's status, and what it plainly is —
// the app's HTML, the source's JSON, a tile's packed bytes once and not twice,
// the same bytes off the edge cache's hit, an empty page left empty, an
// asset's type. What depends on the edge's mood (whether the second ask hit
// the cache) is said, not failed.
//
//   ./script/smoke.ts https://plei.me
//   ./script/smoke.ts https://api.plei.me --origin

const SPEC = new URL("../spec/spec/", import.meta.url);
const read = async (path: string) => JSON.parse(await Deno.readTextFile(new URL(path, SPEC)));

/** The roots and their branches, as route.json names them. */
const route = await read("route.json");
const root = (name: string): string => route.roots[name].path;
const branch = (name: string): string => `${root(route.branches[name].under)}/${route.branches[name].name}`;
const [GEO_ROOT, API_ROOT, TILE_ROOT] = [root("GEO_ROOT"), root("API_ROOT"), root("TILE_ROOT")];
const [SEARCH_URL, GEO_INTERNAL, API_INTERNAL] = [branch("SEARCH_URL"), branch("GEO_INTERNAL"), branch("API_INTERNAL")];
/** A tile's object under its kind, route.json's `object` filled in. */
const tileObject = (version: string, name: string): string => route.roots.TILE_ROOT.object.replace("{version}", version).replace("{name}", name);

/** A land page in Bundang: the zoom-15 tile's name. */
const PAGE = "010332102130223";

/** PAGE's divisions page at z5, its first five digits: one zoom under the
 * pyramid's top (pxl-spec-core lib.rs `TOP_ZOOM`). Written out, since
 * computing it would copy pxl-spec's arithmetic into TypeScript. */
const DIVISION_TOP = "01033";

/** A zoom-15 page in the Arctic Ocean, the pyramid's north-east corner: well
 * shaped, and no bake holds it. */
const OPEN_SEA = "333333333333333";

/** Any shipped asset's bucket key: the first ingested.json lists. */
const ASSET_KEY: string = (await read("asset/ingested.json")).files[0].key;

/** What every baked tile is packed in: zstd's frame magic, little-endian. */
const ZSTD_MAGIC = [0x28, 0xb5, 0x2f, 0xfd];
/** An XTP packet's magic, "XT": what a tile's bytes start with once unpacked. */
const XTP_MAGIC = [0x58, 0x54];

const BROWSER_ENCODINGS = "gzip, deflate, br, zstd";

/** An answer as a check reads it. */
interface Answer {
  status: number;
  headers: Headers;
  body: Uint8Array;
}

interface Check {
  path: string;
  want: number;
  why: string;
  /** What is wrong with an answer of the right status, or null. */
  look?: (a: Answer) => string | null;
  /** Read as stored: the browser's encodings offered, and the body left as it
   * came, which this runtime then does not unpack. Only a tile wants it. */
  raw?: boolean;
}

const starts = (b: Uint8Array, magic: number[]) => magic.every((m, k) => b[k] === m);

/** Packed once: the stored zstd frame, or, where this runtime unpacked it, the packet itself. */
function packed(a: Answer): string | null {
  if (starts(a.body, ZSTD_MAGIC) || starts(a.body, XTP_MAGIC)) return null;
  return `neither a zstd frame nor a packet (${[...a.body.slice(0, 4)].map((b) => b.toString(16).padStart(2, "0")).join("")}): packed twice, or not a tile`;
}

const typed = (want: RegExp) => (a: Answer) => {
  const type = a.headers.get("content-type") ?? "";
  return want.test(type) ? null : `content-type ${type || "none"}`;
};

function json(a: Answer): unknown {
  try {
    return JSON.parse(new TextDecoder().decode(a.body));
  } catch {
    return undefined;
  }
}

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
  return [
    { path: "/", want: 200, why: "the app itself, from the Worker's static assets", look: typed(/^text\/html/) },
    {
      path: `${TILE_ROOT}/source`,
      want: 200,
      why: "the source's declaration, out of the bucket",
      look: (a) => {
        const s = json(a) as { version?: Record<string, unknown> } | undefined;
        const missing = ["terrain", "place", "division", "earth"].filter((k) => typeof s?.version?.[k] !== "string");
        return s === undefined ? "not JSON" : missing.length > 0 ? `no version for ${missing.join(", ")}` : null;
      },
    },
    {
      path: `${SEARCH_URL}?q=${encodeURIComponent("station")}`,
      want: 200,
      why: "pxl-geo, the one geo query with no page to bake",
      look: (a) => Array.isArray(json(a)) ? null : "not a JSON list",
    },
    { path: `${TILE_ROOT}/terrain/${tileObject(version.terrain, PAGE)}`, want: 200, why: "a terrain page, out of the bucket", look: packed, raw: true },
    { path: `${TILE_ROOT}/division/${tileObject(version.division, DIVISION_TOP)}`, want: 200, why: "the divisions' top page over PAGE", look: packed, raw: true },
    {
      path: `${TILE_ROOT}/terrain/${tileObject(version.terrain, OPEN_SEA)}`,
      want: 204,
      why: "a page no bake holds is answered empty — HTML here means `run_worker_first` is gone",
      look: (a) => a.body.length === 0 ? null : `${a.body.length}B of body`,
      raw: true,
    },
    {
      path: `/${ASSET_KEY}`,
      want: 200,
      why: "a content-addressed asset, its own root",
      look: (a) => typed(/^(model\/gltf-binary|image\/(png|webp))/)(a) ?? (a.headers.get("content-encoding") ? `content-encoding ${a.headers.get("content-encoding")}` : null),
    },
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
    { path: `${SEARCH_URL}?q=${encodeURIComponent("station")}`, want: 200, why: "pxl-geo, reached through Caddy without the edge" },
    ...commonChecks(),
    { path: `${GEO_ROOT}/nothing-is-here`, want: 404, why: "pxl-geo answers its root and not this" },
  ];
}

async function ask(url: string, raw = false): Promise<Answer | string> {
  try {
    const res = await fetch(url, { redirect: "manual", headers: raw ? { "accept-encoding": BROWSER_ENCODINGS } : {} });
    return { status: res.status, headers: res.headers, body: new Uint8Array(await res.arrayBuffer()) };
  } catch (err) {
    return err instanceof Error ? err.name : "failed";
  }
}

const argv = Deno.args.filter((a) => a !== "--origin");
const originOnly = Deno.args.includes("--origin");
const base = argv[0]?.replace(/\/$/, "");
if (!base) {
  console.error("usage: deno run -A script/smoke.ts <base-url> [--origin]");
  Deno.exit(2);
}

const version = originOnly ? {} : await servedVersions(base);
const checks = originOnly ? originChecks() : edgeChecks(version);
console.log(`[smoke] ${base}${originOnly ? " (origin, behind the edge)" : ""} — ${checks.length} checks\n`);

let failed = 0;
const line = (mark: string, got: string, path: string, note: string) => console.log(`  ${mark} ${got.padEnd(5)} ${path.length > 58 ? path.slice(0, 55) + "…" : path.padEnd(58)} ${note}`);

for (const { path, want, why, look, raw } of checks) {
  // A unique query string, so a cached answer cannot pass for a dead deployment.
  const a = await ask(`${base}${path}${path.includes("?") ? "&" : "?"}_smoke=${crypto.randomUUID()}`, raw);
  const wrong = typeof a === "string" ? a : a.status !== want ? `expected ${want}` : look?.(a) ?? null;
  if (wrong !== null) failed++;
  line(wrong === null ? "ok  " : "FAIL", typeof a === "string" ? "-" : String(a.status), path, wrong === null ? `${(a as Answer).body.length}B` : `${wrong} — ${why}`);
}

// The edge cache's hit leaves by its own path (the Worker's `STORED_ENCODING`),
// so a first answer can be right and every hit after it wrong: one tile asked
// twice at one URL must come back as the same bytes.
if (!originOnly) {
  const path = `${TILE_ROOT}/terrain/${tileObject(version.terrain, PAGE)}`;
  const url = `${base}${path}?_smoke=${crypto.randomUUID()}`;
  const [first, second] = [await ask(url, true), await ask(url, true)];
  if (typeof first === "string" || typeof second === "string") {
    failed++;
    line("FAIL", "-", `${path} ×2`, "a request failed");
  } else {
    const same = first.status === second.status && first.body.length === second.body.length && first.body.every((b, k) => b === second.body[k]) &&
      first.headers.get("content-encoding") === second.headers.get("content-encoding");
    const wrong = !same ? "the second answer differs from the first: the cache's way out is broken" : packed(second);
    if (wrong !== null) failed++;
    line(wrong === null ? "ok  " : "FAIL", String(second.status), `${path} ×2`, wrong ?? `${second.body.length}B twice`);
    const hit = second.headers.get("cf-cache-status");
    if (wrong === null && hit !== "HIT") console.log(`  warn       the second ask was not a cache hit (cf-cache-status ${hit ?? "none"}): its way out went unchecked`);
  }
}

console.log();
if (failed > 0) {
  console.error(`[smoke] ${failed} failed`);
  Deno.exit(1);
}
console.log(`[smoke] all answered as they should`);
