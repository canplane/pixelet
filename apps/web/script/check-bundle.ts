#!/usr/bin/env -S deno run -A
// Fails a production build whose bundle still carries dev-only material:
// dev catalog strings, dev markers, dev settings rows, bundled models.
//
//   deno run -A script/check-bundle.ts   (from apps/web, after a build)

import { SETTINGS, SETTINGS_SHIPPED } from "../src/feature/setting/schema.spec.ts";

const DIST = "./dist";

const DEV_LOCALE_FILES = [
  "./src/locale/dev/en.json",
  "./src/locale/dev/ko.json",
];

/** A dev string also shipped here (e.g. "Places") is not evidence of a leak. */
const SHIPPED_LOCALE_FILES = [
  "./src/locale/en.json",
  "./src/locale/ko.json",
];

/** "development" is __APP_ENV__'s value; a production build once shipped it. */
const DEV_MARKERS = ["dev/test", "development"];

/** Proves the check reads a real bundle, so an empty dist cannot pass. Each
 * is a literal the minifier keeps, in the artifact that requests it; the
 * spec's `bundle` flag names which, and pxl-tool/spec/route.rs checks it. */
const SENTINELS = ["/geo/search", "/tile"];
// Kept empty: the generator fails if this list is missing.
const KERNEL_ROUTE_SENTINELS = [] as string[];
const KERNEL_SENTINELS = ["_ocean"];

function leaves(o: unknown, path = ""): [string, string][] {
  if (o !== null && typeof o === "object") {
    return Object.entries(o as Record<string, unknown>)
      .flatMap(([k, v]) => leaves(v, path ? `${path}.${k}` : k));
  }
  return typeof o === "string" ? [[path, o]] : [];
}

async function readBundle(): Promise<string> {
  let out = "";
  let count = 0;
  for await (const entry of Deno.readDir(`${DIST}/assets`)) {
    if (!entry.isFile) continue;
    if (!entry.name.endsWith(".js") && !entry.name.endsWith(".css")) continue;
    out += await Deno.readTextFile(`${DIST}/assets/${entry.name}`);
    count++;
  }
  if (count === 0) throw new Error(`no bundle files under ${DIST}/assets — build first`);
  console.log(`[check-bundle] scanning ${count} file(s)`);
  return out;
}

/** Read one byte to a character: its string data lies in it uncompressed. */
async function readKernel(): Promise<string> {
  const modules: string[] = [];
  for await (const entry of Deno.readDir(`${DIST}/assets`)) {
    if (entry.isFile && entry.name.endsWith(".wasm")) modules.push(entry.name);
  }
  if (modules.length !== 1) throw new Error(`expected one kernel module under ${DIST}/assets, found ${modules.length}`);
  return new TextDecoder("latin1").decode(await Deno.readFile(`${DIST}/assets/${modules[0]}`));
}

const bundle = await readBundle();
const kernel = await readKernel();

/** esbuild quotes and oxc (Vite 8) writes backticks; both count. */
function quoted(text: string, v: string): boolean {
  return text.includes(JSON.stringify(v)) || text.includes(`'${v}'`) || text.includes(`\`${v}\``);
}

for (const s of SENTINELS) {
  if (!quoted(bundle, s)) {
    console.error(`[check-bundle] sentinel ${JSON.stringify(s)} missing from the bundle — the build is not what this expects, so a pass would mean nothing`);
    Deno.exit(1);
  }
}
for (const s of [...KERNEL_ROUTE_SENTINELS, ...KERNEL_SENTINELS]) {
  if (!kernel.includes(s)) {
    console.error(`[check-bundle] sentinel ${JSON.stringify(s)} missing from the kernel — the build is not what this expects, so a pass would mean nothing`);
    Deno.exit(1);
  }
}

const forbidden: [string, string][] = [...DEV_MARKERS.map((m): [string, string] => ["marker", m])];
for (const f of DEV_LOCALE_FILES) {
  forbidden.push(...leaves(JSON.parse(await Deno.readTextFile(f))).map(([p, v]): [string, string] => [`${f}:${p}`, v]));
}

/** Only quoted literals count: a bare "Debug" matched `useDebugValue`.
 * Shorter strings collide even quoted, so they are skipped. */
const MIN_LEN = 4;

function present(bundle: string, v: string): boolean {
  if (v.length < MIN_LEN) return false;
  return quoted(bundle, v);
}

const shipped = new Set<string>();
for (const f of SHIPPED_LOCALE_FILES) {
  for (const [, v] of leaves(JSON.parse(await Deno.readTextFile(f)))) shipped.add(v);
}
const shared = (where: string, v: string) => where !== "marker" && shipped.has(v);
const leaked = forbidden.filter(([where, v]) => present(bundle, v) && !shared(where, v));

if (leaked.length > 0) {
  console.error(`[check-bundle] ${leaked.length} development-only string(s) present in the production bundle:`);
  for (const [where, v] of leaked) {
    console.error(`  ${where}\n    ${JSON.stringify(v.slice(0, 60))}`);
  }
  console.error("\nA dev surface is gated by __IS_DEV__ but something attached to it is not.");
  Deno.exit(1);
}

const excused = forbidden.filter(([where, v]) => shared(where, v)).length;
console.log(`[check-bundle] ok — ${forbidden.length} development-only string(s), none present${excused > 0 ? ` (${excused} the shipped catalogs carry too)` : ""}`);

// A dev setting's row: the settings table is generated whole from
// schema.json, and a build that filtered it at run time shipped every dev
// row under its name — `shadows:{kind:...}` and the rest,
// readable in the artifact with their defaults. The shell now takes the
// table without them (feature/setting/schema.ts), and this reads the
// artifact to see that it did. What is looked for is the row and not the
// bare name: a name may be read elsewhere for its own reasons, and a row is
// a property whose value is
// an object. A row every build offers is the sentinel, so that a minifier
// writing rows some other way fails this loudly rather than passing it
// blind.
const SETTINGS_SPEC = "../../spec/pxl/web/kernel/shell/setting/schema.json";
const SETTING_ROW_SENTINEL = "structures";

function row(bundle: string, key: string): boolean {
  return new RegExp(`[{,]\\s*["'\`]?${key}["'\`]?\\s*:\\s*\\{`).test(bundle);
}

if (!row(bundle, SETTING_ROW_SENTINEL)) {
  console.error(`[check-bundle] sentinel row ${JSON.stringify(SETTING_ROW_SENTINEL)} missing from the bundle — the settings table is not written the way this reads it, so a pass would mean nothing`);
  Deno.exit(1);
}
const shippedRows = devRows.filter((k) => row(bundle, k));
if (shippedRows.length > 0) {
  console.error(`[check-bundle] ${shippedRows.length} development-only setting row(s) present in the production bundle:`);
  for (const k of shippedRows) console.error(`  ${k}`);
  console.error("\nfeature/setting/schema.ts is meant to take SETTINGS_SHIPPED under a production build; the bundler kept the whole table.");
  Deno.exit(1);
}
console.log(`[check-bundle] ok — ${devRows.length} development-only setting row(s), none present`);

// The published kernel is built without the `dev` feature, so its rows are absent too.
const named = devRows.filter((k) => kernel.includes(k));
if (named.length > 0) {
  console.error(`[check-bundle] ${named.length} development-only setting name(s) in the kernel module:`);
  for (const k of named) console.error(`  ${k}`);
  console.error("\n`build` is meant to bundle the module build-wasm.ts makes with --published; this one still has the table's dev rows.");
  Deno.exit(1);
}
console.log(`[check-bundle] ok — none of the ${devRows.length} named in the kernel module`);

// Every model and texture is fetched from the bucket; a `?url` import would bundle it.
const carried: string[] = [];
for await (const entry of Deno.readDir(`${DIST}/assets`)) {
  if (entry.isFile && /\.(glb|gltf|png|jpe?g|webp)$/.test(entry.name)) carried.push(entry.name);
}
if (carried.length > 0) {
  console.error(`[check-bundle] ${carried.length} asset file(s) in the production bundle that belong in the bucket:`);
  for (const name of carried) console.error(`  ${name}`);
  console.error("\nModels and textures are ingested (deno task ingest:asset) and named by the palette, not imported.");
  Deno.exit(1);
}
console.log("[check-bundle] ok — no model or texture bytes in the bundle");
