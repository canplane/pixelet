#!/usr/bin/env -S deno run -A
// Fails a production build that still carries development-only material.
//
// Gating a dev surface behind __IS_DEV__ removes the *component*, but not
// necessarily everything attached to it: the debug settings section was
// eliminated correctly while its labels rode along inside the bundled i18n
// catalogue, so a production bundle named every debug toggle it offered. And
// before that, `VITE_APP_ENV=development` in a local .env — which Vite loads
// for every mode — made __IS_DEV__ true in the deployed build outright.
//
// Both were found by reading the built bundle, so that is what this does.
// "Unreachable at runtime" is not the bar; absent from the artifact is.
//
// Usage (from apps/web, after a production build):
//   deno run -A script/check-bundle.ts

const DIST = "./dist";

/** Files whose every leaf string must not appear in a production bundle. The
 * dev catalogues are the authoritative list of dev-only copy, so deriving the
 * check from them means a new dev string is covered the day it is added. */
const DEV_LOCALE_FILES = [
  "./src/locale/dev/en.json",
  "./src/locale/dev/ko.json",
];

/** The catalogues a production build is supposed to carry. A string one of
 * these holds cannot be evidence that the dev catalogue leaked — the bundle
 * is meant to have it — so a dev string that is also one of these is not
 * counted against the build. `Places` is the case that found this: the
 * attribution section names Overture's Places theme, and the dev row for the
 * place badges is the same word. What is only ever dev-only still fires, and
 * a whole catalogue riding along would light up every string of it that is
 * not shared, which is nearly all of them. */
const SHIPPED_LOCALE_FILES = [
  "./src/locale/en.json",
  "./src/locale/ko.json",
];

/** Markers that are not copy — route paths and the like.
 *
 * "development" is __APP_ENV__'s value, which the settings panel prints. A
 * production build shipped it once: the guard that forces a production build
 * out of development mode had been written into the isDev boolean only, so
 * the name kept whatever .env said and the deployed panel labelled itself
 * "development" while the debug section it implies was correctly absent.
 * Confirmed absent from a correct production bundle, so it is safe to forbid
 * outright rather than to check contextually. */
const DEV_MARKERS = ["dev/test", "development"];

/** Strings that prove the check is looking at a real bundle. Without this a
 * broken glob or an empty dist would pass silently, which is the failure mode
 * a guard like this most easily falls into.
 *
 * These have to be literals that survive minification, not paths the app
 * composes at runtime, and what survives is the whole branch the client
 * requests rather than the root it hangs off: spec/pxl/spec/route.json folds a branch
 * into one constant, so `"/geo/tiles"` reaches an artifact and `"/geo"` never
 * does.
 *
 * Which artifact is the point of the split. A path is a literal in whichever
 * of the two asks for it, and asking moves — it has moved twice. The tiles
 * branch was the shell's until the kernel took over composing a page's URL, at
 * which point the shell's copy had no consumer, the bundler dropped it, and
 * this failed a deploy over a path that was present all along, in the other
 * file (2026-09-23). It is the shell's again now that the kernel names an
 * object and never an address (io/fetch.rs), and this failed a deploy the
 * other way round (2026-09-24). So the spec's `bundle` flag names the artifact
 * rather than asserting there is one, and script/spec.ts fails when either
 * list here and the flag disagree.
 *
 * Neither failure was caught before the deploy, because this runs there and
 * not in `deno task check`: it reads a built `dist/`, which the gate does not
 * produce.
 *
 * KERNEL_SENTINELS is what no route names: the terrain's page names are the
 * kernel's own (world/pyramid.rs), so nothing generates them
 * and there is nothing to check them against. */
const SENTINELS = ["/geo/search", "/geo/tiles"];
// Empty, and kept rather than deleted: the generator's error when this list is
// missing is what would catch a branch marked for the kernel with nothing here
// to check it.
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

/** The kernel's module, as bytes read one to a character: its string data
 * lies in it uncompressed. */
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

/** A string as a bundler may have written it. Which delimiter comes out is
 * the minifier's own choice and it does change: esbuild quotes, and oxc —
 * what Vite bundles with from 8 — writes backticks, so every check here went
 * blind on a bundle that was carrying the strings all along. The sentinels
 * are what caught that, which is what they are for. */
function quoted(text: string, v: string): boolean {
  return text.includes(JSON.stringify(v)) || text.includes(`'${v}'`) || text.includes(`\`${v}\``);
}

for (const s of SENTINELS) {
  if (!quoted(bundle, s)) {
    console.error(`[check-bundle] sentinel ${JSON.stringify(s)} missing from the bundle — the build is not what this expects, so a pass would mean nothing`);
    Deno.exit(1);
  }
}
// Raw bytes, not a literal in source: the kernel's strings lie in its data.
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

/**
 * Matches the quoted literal, not the bare substring. A bare search flagged
 * the English string "Debug" because React's `useDebugValue` contains those
 * letters — short dictionary words will collide with minified identifiers
 * every time. A catalogue string reaches the bundle as a string literal, so
 * requiring its delimiters is both stricter and closer to what is being
 * asked.
 *
 * Very short strings are skipped regardless: at that length a collision is
 * likely even quoted, and a false failure that blocks deploys would get this
 * check disabled rather than fixed.
 */
const MIN_LEN = 4;

function present(bundle: string, v: string): boolean {
  if (v.length < MIN_LEN) return false;
  return quoted(bundle, v);
}

const shipped = new Set<string>();
for (const f of SHIPPED_LOCALE_FILES) {
  for (const [, v] of leaves(JSON.parse(await Deno.readTextFile(f)))) shipped.add(v);
}
// A marker is not copy, so the catalogues cannot excuse one.
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
console.log(`[check-bundle] ok — ${forbidden.length} development-only string(s), none present${excused > 0 ? ` (${excused} the shipped catalogues carry too)` : ""}`);

// A dev setting's row: the settings table is generated whole from
// schema.json, and a build that filtered it at run time shipped every dev
// row under its name — `enable_terrain_carving:{kind:...}` and the rest,
// readable in the artifact with their defaults. The shell now takes the
// table without them (feature/setting/schema.ts), and this reads the
// artifact to see that it did. What is looked for is the row and not the
// bare name: a name may be read elsewhere for its own reasons (the ledger
// report has an `msaa` of its own), and a row is a property whose value is
// an object. A row every build offers is the sentinel, so that a minifier
// writing rows some other way fails this loudly rather than passing it
// blind.
const SETTINGS_SPEC = "../../spec/pxl/web/kernel/shell/setting/schema.json";
const SETTING_ROW_SENTINEL = "show_structures";

function row(bundle: string, key: string): boolean {
  return new RegExp(`[{,]\\s*["'\`]?${key}["'\`]?\\s*:\\s*\\{`).test(bundle);
}

const settingsSpec = JSON.parse(await Deno.readTextFile(SETTINGS_SPEC)) as { SETTINGS: Record<string, { dev?: boolean }> };
if (!row(bundle, SETTING_ROW_SENTINEL)) {
  console.error(`[check-bundle] sentinel row ${JSON.stringify(SETTING_ROW_SENTINEL)} missing from the bundle — the settings table is not written the way this reads it, so a pass would mean nothing`);
  Deno.exit(1);
}
const devRows = Object.entries(settingsSpec.SETTINGS).filter(([, s]) => s.dev === true).map(([k]) => k);
const shippedRows = devRows.filter((k) => row(bundle, k));
if (shippedRows.length > 0) {
  console.error(`[check-bundle] ${shippedRows.length} development-only setting row(s) present in the production bundle:`);
  for (const k of shippedRows) console.error(`  ${k}`);
  console.error("\nfeature/setting/schema.ts is meant to take SETTINGS_SHIPPED under a production build; the bundler kept the whole table.");
  Deno.exit(1);
}
console.log(`[check-bundle] ok — ${devRows.length} development-only setting row(s), none present`);

// The kernel's table has the same rows, and the module `build` bundles is
// built without them (pxl-kernel's `dev` feature, off under build-wasm.ts's
// `--published`), so their names are not in its data either. Raw bytes, as
// the kernel's sentinel is.
const named = devRows.filter((k) => kernel.includes(k));
if (named.length > 0) {
  console.error(`[check-bundle] ${named.length} development-only setting name(s) in the kernel module:`);
  for (const k of named) console.error(`  ${k}`);
  console.error("\n`build` is meant to bundle the module build-wasm.ts makes with --published; this one still has the table's dev rows.");
  Deno.exit(1);
}
console.log(`[check-bundle] ok — none of the ${devRows.length} named in the kernel module`);

// A model or a texture in the bundle is a file the deploy is carrying that
// the bucket should — every one the client draws is named by the shipped
// palette and fetched by content key. A `?url` import of a
// .glb or .png would put it back here silently, so the artifact is checked
// rather than the source.
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
