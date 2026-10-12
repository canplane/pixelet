#!/usr/bin/env -S deno run -A
// Uploads each kind's `out/<kind>.publish/` as a new version and sets it, with
// the format and shards its set's manifest names, in published.json's
// `SOURCE.kind`; then uploads the declaration pxl-geo-bake writes of `SOURCE`
// as the bucket's `source`, which clients read at boot: a publish needs no
// deploy. A version the declaration would refuse goes up not at all. The
// name is `v<format>.<UTC day>.<n>`, n past every version of that format and
// day the bucket holds, so no version is written over. `source` alone uploads
// the declaration as `SOURCE` stands, after an override is written by hand.
// Uses the AWS CLI's `r2` profile and `$R2`, the bucket's S3 endpoint
// (publish-bucket).
//
//   ./script/publish.ts <terrain|place|division|earth|source>…

import { tileObject } from "../apps/web/src/config/route.spec.ts";

const BUCKET = "pixelet-tiles";
const PUBLISHED = "spec/spec/bake/published.json";
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

/** The declaration pxl-geo-bake writes of a published.json (bin/declare_source.rs), refusing what a client would. */
async function declared(published: string): Promise<Uint8Array> {
  const out = await new Deno.Command("cargo", {
    args: ["run", "-q", "--manifest-path", "crates/Cargo.toml", "-p", "pxl-geo-bake", "--bin", "declare_source", "--", published],
    stderr: "inherit",
  }).output();
  return out.success ? out.stdout : die("declare_source refused it");
}

async function publish(kind: string): Promise<void> {
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
  const published = JSON.parse(Deno.readTextFileSync(PUBLISHED));
  published.SOURCE.kind[kind] = { version: name, format, ...(shard ? { shard } : {}) };
  const text = `${JSON.stringify(published, null, 2)}\n`;
  // Refused before a byte goes up: a version no declaration can name serves no one.
  await withFile(new TextEncoder().encode(text), declared);

  const objects = files(dir);
  console.log(`[publish/${kind}] ${objects} objects to ${kind}/${name}`);
  // Packed already: the encoding goes with each object (publish-bucket "Upload").
  await aws([
    "s3",
    "sync",
    dir,
    `s3://${BUCKET}/${kind}/${tileObject(name, "")}`,
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

  const stored = (await aws(["s3", "ls", `s3://${BUCKET}/${kind}/${tileObject(name, "")}`, "--recursive"])).split("\n").filter((l) => l.trim()).length;
  if (stored !== objects) die(`${kind}/${name}: ${stored} objects stored of ${objects}; re-run the sync by hand before pointing the client at it`);

  Deno.writeTextFileSync(PUBLISHED, text);
  Deno.removeSync(dir, { recursive: true });
  console.log(`[publish/${kind}] SOURCE.kind.${kind}.version = ${name}`);
}

/** Revalidated on every boot, so a publish is seen at once. */
async function publishSource(): Promise<void> {
  await withFile(await declared(PUBLISHED), (file) => aws(["s3", "cp", file, `s3://${BUCKET}/${SOURCE}`, "--content-type", "application/json", "--cache-control", "no-cache"], true));
  console.log(`[publish] ${SOURCE} uploaded`);
}

try {
  const asked = Deno.args;
  if (asked.length === 0 || asked.some((k) => !KINDS.includes(k) && k !== SOURCE)) die(`usage: ./script/publish.ts <${[...KINDS, SOURCE].join("|")}>…`);
  endpoint();
  for (const kind of asked.filter((k) => k !== SOURCE)) await publish(kind);
  await publishSource();
} catch (e) {
  if (!(e instanceof Refused)) throw e;
  console.error(`[publish] ${e.message}`);
  Deno.exit(1);
}
