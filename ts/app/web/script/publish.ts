#!/usr/bin/env -S deno run -A
// Publishes to the bucket, and only there: the bucket's `source`, which
// clients read at boot, is the one record of which version of each kind is
// served, and each version holds its own manifest beside its tiles, its format
// and shards. A publish reads both out of the bucket, makes the declaration
// of them (pxl-geo-bake bin/declare_source.rs, refusing what a client would)
// and uploads it; nothing in the tree changes, so nothing is deployed, and
// the one thing built is that declaring tool.
//
// `upload <kind>` uploads `out/<kind>.publish/` as a new version, named
// `v<format>.<UTC day>.<n>`, n past every version of that format and day the
// bucket holds, so no version is written over, and serves nothing: what the
// clients deployed read stays as it is until a version is served. `<kind>`
// uploads and serves it. `serve <kind> <version>` serves a version the bucket
// holds, as a new format's is with the client that reads it, and going back
// is. `override <shard> <kind> <version>` serves that version for a shard
// and the tiles under it, `-` dropping it. Uses the AWS CLI's `r2` profile
// and `$R2`, the bucket's S3 endpoint (publish-bucket).
//
//   ./ts/app/web/script/publish.ts <terrain|place|division|earth>…
//   ./ts/app/web/script/publish.ts upload <terrain|place|division|earth>…
//   ./ts/app/web/script/publish.ts serve <kind> <version>
//   ./ts/app/web/script/publish.ts override <shard> <kind> <version|->

import { tileObject } from "@pxl/client/config/route.spec.ts";

const BUCKET = "pixelet-tiles";
const KINDS = ["terrain", "place", "division", "earth"];
const SOURCE = "source";

/** Thrown, so what is open is closed on the way out; said once at the top. */
class Refused extends Error {}

function die(message: string): never {
  throw new Refused(message);
}

function endpoint(): string {
  return Deno.env.get("R2") || die("no $R2: export the bucket's S3 endpoint (publish-bucket)");
}
// AWS CLI v2 sends checksum headers R2 refuses.
const env = { AWS_REQUEST_CHECKSUM_CALCULATION: "when_required" };

async function aws(args: string[], inherit = false, none = false): Promise<string> {
  const out = await new Deno.Command("aws", {
    args: [...args, "--profile", "r2", "--endpoint-url", endpoint()],
    env,
    stdout: inherit ? "inherit" : "piped",
    stderr: inherit ? "inherit" : "piped",
  }).output();
  // `s3 ls` exits 1, silent, on a prefix that holds nothing.
  if (none && out.code === 1 && out.stderr.length === 0) return "";
  if (!out.success) die(`aws ${args.slice(0, 3).join(" ")} failed${inherit ? "" : `:\n${new TextDecoder().decode(out.stderr)}`}`);
  return inherit ? "" : new TextDecoder().decode(out.stdout);
}

/** Beside a copy's tiles, what it is: never uploaded. */
const MANIFEST = "manifest.json";

/** The files under `dir`, as the sync uploads them. */
function files(dir: string): number {
  let n = 0;
  for (const e of Deno.readDirSync(dir)) {
    if (e.isDirectory) n += files(`${dir}/${e.name}`);
    else if (!e.name.endsWith(".part") && e.name !== MANIFEST) n += 1;
  }
  return n;
}

/** The versions the bucket holds under `kind`. */
async function versions(kind: string): Promise<string[]> {
  const listed = await aws(["s3", "ls", `s3://${BUCKET}/${kind}/`], false, true);
  return listed.split("\n").map((line) => line.trim().match(/^PRE (.+)\/$/)?.[1] ?? "").filter(Boolean);
}

/** Past every `v<format>.<day>.<n>` the bucket holds under `kind`. */
async function nextName(kind: string, format: number, day: string): Promise<string> {
  const prefix = `v${format}.${day}.`;
  const taken = (await versions(kind)).filter((name) => name.startsWith(prefix)).map((name) => Number(name.slice(prefix.length)));
  return `${prefix}${Math.max(0, ...taken) + 1}`;
}

/** `bytes` in a file for `use`, gone after however `use` ends. */
async function withFile<T>(bytes: Uint8Array, use: (file: string) => Promise<T>): Promise<T> {
  const file = await Deno.makeTempFile({ suffix: ".json" });
  try {
    Deno.writeFileSync(file, bytes);
    return await use(file);
  } finally {
    Deno.removeSync(file);
  }
}

interface Kind {
  version: string;
  format: number;
  shard?: string[];
}

/** What the bucket serves: each kind's version as its manifest says it is, and the overrides. */
interface Served {
  kind: { [kind: string]: Kind };
  override: { [shard: string]: { [kind: string]: string } };
}

/** What a client reads at boot. */
interface Declaration {
  version: { [kind: string]: string };
  shard: { [key: string]: { version?: { [kind: string]: string } } };
}

/** The manifest a version holds beside its tiles. */
async function manifestOf(kind: string, version: string): Promise<Kind> {
  const { format, shard } = JSON.parse(await aws(["s3", "cp", `s3://${BUCKET}/${kind}/${tileObject(version, MANIFEST)}`, "-"]));
  return { version, format, ...(shard ? { shard } : {}) };
}

/** What the bucket serves now, out of its `source` and the manifests of the versions it names. */
async function servedNow(): Promise<Served> {
  const declared: Declaration = JSON.parse(await aws(["s3", "cp", `s3://${BUCKET}/${SOURCE}`, "-"]));
  const kind: Served["kind"] = {};
  for (const k of KINDS) kind[k] = await manifestOf(k, declared.version[k] ?? die(`${SOURCE} serves no ${k}`));
  const override: Served["override"] = {};
  for (const [key, { version }] of Object.entries(declared.shard)) if (version) override[key] = version;
  return { kind, override };
}

/** The declaration pxl-geo-bake writes of a record (bin/declare_source.rs), refusing what a client would. */
async function declared(record: Served): Promise<Uint8Array> {
  const child = new Deno.Command("cargo", {
    args: ["run", "-q", "--manifest-path", "rust/Cargo.toml", "-p", "pxl-geo-bake", "--bin", "declare_source"],
    stdin: "piped",
    stdout: "piped",
    stderr: "inherit",
  }).spawn();
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode(JSON.stringify(record)));
  await writer.close();
  const out = await child.output();
  return out.success ? out.stdout : die("declare_source refused it");
}

/** Revalidated on every boot, so a publish is seen at once. */
async function serve(declaration: Uint8Array): Promise<void> {
  await withFile(declaration, (file) => aws(["s3", "cp", file, `s3://${BUCKET}/${SOURCE}`, "--content-type", "application/json", "--cache-control", "no-cache"], true));
  console.log(`[publish] ${SOURCE} uploaded`);
}

/** `out/<kind>.publish/` uploaded as a new version, and what it is: served by
 * no one until a declaration names it (`serve`). */
async function upload(record: Served, kind: string): Promise<Kind> {
  const dir = `out/${kind}.publish`;
  try {
    Deno.statSync(dir);
  } catch {
    die(`${kind}: no ${dir}: run \`deno task bake ${kind}\` (with \`--stage publish\` where it has stages)`);
  }
  // What the version is, fixed with it: the copy as its own manifest says.
  const manifest = JSON.parse(Deno.readTextFileSync(`${dir}/${MANIFEST}`));
  const { format, shard } = manifest;
  if (!Number.isInteger(format)) die(`${dir}/${MANIFEST} names no format the set is in`);
  const name = await nextName(kind, format, new Date().toISOString().slice(0, 10));
  const version: Kind = { version: name, format, ...(shard ? { shard } : {}) };
  // Refused before a byte goes up: a version no declaration can name serves no one.
  await declared({ ...record, kind: { ...record.kind, [kind]: version } });

  const objects = files(dir);
  const at = `s3://${BUCKET}/${kind}/${tileObject(name, "")}`;
  console.log(`[publish/${kind}] ${objects} objects to ${kind}/${name}`);
  // Packed already: the encoding goes with each object (publish-bucket "Upload").
  await aws([
    "s3",
    "sync",
    dir,
    at,
    "--exclude",
    "*.part",
    "--exclude",
    MANIFEST,
    "--content-type",
    manifest.content_type,
    "--content-encoding",
    manifest.content_encoding,
    "--cache-control",
    "public, max-age=31536000, immutable",
  ], true);
  const stored = (await aws(["s3", "ls", at, "--recursive"])).split("\n").filter((l) => l.trim()).length;
  if (stored !== objects) die(`${kind}/${name}: ${stored} objects stored of ${objects}; re-run the sync by hand before serving it`);
  // Last, so a version with a manifest is one whole.
  await aws(["s3", "cp", `${dir}/${MANIFEST}`, `${at}${MANIFEST}`, "--content-type", "application/json"], true);

  Deno.removeSync(dir, { recursive: true });
  console.log(`[publish/${kind}] ${kind}/${name} uploaded`);
  return version;
}

/** What `args` changes of what the bucket serves, `record`: the declaration
 * to serve, or none where nothing is served (`upload`). */
async function command(record: Served, [first, ...rest]: string[]): Promise<Served | undefined> {
  if (first === "serve") {
    const [kind, version] = rest;
    if (!KINDS.includes(kind) || !version) die("usage: ./ts/app/web/script/publish.ts serve <kind> <version>");
    record.kind[kind] = await manifestOf(kind, version);
  } else if (first === "override") {
    const [shard, kind, version] = rest;
    if (!shard || !KINDS.includes(kind) || !version) die("usage: ./ts/app/web/script/publish.ts override <shard> <kind> <version|->");
    const own = { ...record.override[shard] };
    if (version === "-") delete own[kind];
    else own[kind] = version;
    if (Object.keys(own).length > 0) record.override[shard] = own;
    else delete record.override[shard];
  } else if (first === "upload") {
    if (rest.length === 0 || rest.some((k) => !KINDS.includes(k))) die(`usage: ./ts/app/web/script/publish.ts upload <${KINDS.join("|")}>…`);
    for (const kind of rest) await upload(record, kind);
    return undefined;
  } else {
    const kinds = [first, ...rest];
    if (!first || kinds.some((k) => !KINDS.includes(k))) {
      die(`usage: ./ts/app/web/script/publish.ts [upload] <${KINDS.join("|")}>… | serve <kind> <version> | override <shard> <kind> <version|->`);
    }
    for (const kind of kinds) record.kind[kind] = await upload(record, kind);
  }
  return record;
}

try {
  endpoint();
  const served = await command(await servedNow(), Deno.args);
  if (served) await serve(await declared(served));
} catch (e) {
  if (!(e instanceof Refused)) throw e;
  console.error(`[publish] ${e.message}`);
  Deno.exit(1);
}
