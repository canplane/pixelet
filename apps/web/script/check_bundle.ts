// @pxl/web/../script/check_bundle.ts
//
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
//   deno run -A script/check_bundle.ts

const DIST = "./dist";

/** Files whose every leaf string must not appear in a production bundle. The
 * dev catalogues are the authoritative list of dev-only copy, so deriving the
 * check from them means a new dev string is covered the day it is added. */
const DEV_LOCALE_FILES = [
  "./src/locale/dev/en.json",
  "./src/locale/dev/ko.json",
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
 * composes at runtime: `/api/tiles` is built from a template literal that
 * esbuild leaves unfolded, so it never appears contiguously. The shell's
 * sentinel is looked for in the JavaScript, the kernel's in its module: the
 * terrain's page names are the kernel's (layer/terrain/tile_repository.rs). */
const SENTINELS = ['"/geo"', '"/api"', '"/asset"'];
const KERNEL_SENTINELS = ["_ocean"];

function leaves(o: unknown, path = ""): [string, string][] {
  if (o !== null && typeof o === "object") {
    return Object.entries(o as Record<string, unknown>)
      .flatMap(([k, v]) => leaves(v, path ? `${path}.${k}` : k));
  }
  return typeof o === "string" ? [[path, o]] : [];
}

async function read_bundle(): Promise<string> {
  let out = "";
  let count = 0;
  for await (const entry of Deno.readDir(`${DIST}/assets`)) {
    if (!entry.isFile) continue;
    if (!entry.name.endsWith(".js") && !entry.name.endsWith(".css")) continue;
    out += await Deno.readTextFile(`${DIST}/assets/${entry.name}`);
    count++;
  }
  if (count === 0) throw new Error(`no bundle files under ${DIST}/assets — build first`);
  console.log(`[check_bundle] scanning ${count} file(s)`);
  return out;
}

/** The kernel's module, as bytes read one to a character: its string data
 * lies in it uncompressed. */
async function read_kernel(): Promise<string> {
  const modules: string[] = [];
  for await (const entry of Deno.readDir(`${DIST}/assets`)) {
    if (entry.isFile && entry.name.endsWith(".wasm")) modules.push(entry.name);
  }
  if (modules.length !== 1) throw new Error(`expected one kernel module under ${DIST}/assets, found ${modules.length}`);
  return new TextDecoder("latin1").decode(await Deno.readFile(`${DIST}/assets/${modules[0]}`));
}

const bundle = await read_bundle();
const kernel = await read_kernel();

for (const [where, text, sentinels] of [["bundle", bundle, SENTINELS], ["kernel", kernel, KERNEL_SENTINELS]] as const) {
  for (const s of sentinels) {
    if (!text.includes(s)) {
      console.error(`[check_bundle] sentinel ${JSON.stringify(s)} missing from the ${where} — the build is not what this expects, so a pass would mean nothing`);
      Deno.exit(1);
    }
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
 * asked. Both quote styles, since which one a minifier emits is its choice.
 *
 * Very short strings are skipped regardless: at that length a collision is
 * likely even quoted, and a false failure that blocks deploys would get this
 * check disabled rather than fixed.
 */
const MIN_LEN = 4;

function present(bundle: string, v: string): boolean {
  if (v.length < MIN_LEN) return false;
  return bundle.includes(JSON.stringify(v)) || bundle.includes(`'${v}'`);
}

const leaked = forbidden.filter(([, v]) => present(bundle, v));

if (leaked.length > 0) {
  console.error(`[check_bundle] ${leaked.length} development-only string(s) present in the production bundle:`);
  for (const [where, v] of leaked) {
    console.error(`  ${where}\n    ${JSON.stringify(v.slice(0, 60))}`);
  }
  console.error("\nA dev surface is gated by __IS_DEV__ but something attached to it is not.");
  Deno.exit(1);
}

console.log(`[check_bundle] ok — ${forbidden.length} development-only string(s), none present`);

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
  console.error(`[check_bundle] ${carried.length} asset file(s) in the production bundle that belong in the bucket:`);
  for (const name of carried) console.error(`  ${name}`);
  console.error("\nModels and textures are ingested (deno task ingest:asset) and named by the palette, not imported.");
  Deno.exit(1);
}
console.log("[check_bundle] ok — no model or texture bytes in the bundle");
