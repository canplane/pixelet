#!/usr/bin/env -S deno run -A
// Uploads each kind's `out/<kind>.publish/` as a new version, sets it and the
// shards its set's manifest holds in published.json's `SOURCE.kind`, and
// uploads `SOURCE` as the bucket's `source`, which clients read at boot: a
// publish needs no deploy. The name
// is `v<format>.<UTC day>.<n>`: the format is published.json's `FORMAT`, set
// by hand; n is past every version of that format and day the bucket holds,
// so no version is written over. `source` alone uploads `SOURCE` as it
// stands, after an edit by hand (an override, the Earth's path). Uses the AWS CLI's `r2` profile and `$R2`,
// the bucket's S3 endpoint (publish-bucket).
//
//   ./script/publish.ts <terrain|place|division|source>…

const BUCKET = "pixelet-tiles";
const PUBLISHED = "spec/spec/bake/published.json";
const KINDS = ["terrain", "place", "division"];
const SOURCE = "source";

function die(message: string): never {
  console.error(`[publish] ${message}`);
  Deno.exit(1);
}

const endpoint = Deno.env.get("R2") || die("no $R2: export the bucket's S3 endpoint (publish-bucket)");
// AWS CLI v2 sends checksum headers R2 refuses.
const env = { AWS_REQUEST_CHECKSUM_CALCULATION: "when_required" };

async function aws(args: string[], inherit = false, none = false): Promise<string> {
  const out = await new Deno.Command("aws", {
    args: [...args, "--profile", "r2", "--endpoint-url", endpoint],
    env,
    stdout: inherit ? "inherit" : "piped",
    stderr: inherit ? "inherit" : "piped",
  }).output();
  // `s3 ls` exits 1, silent, on a prefix that holds nothing.
  if (none && out.code === 1 && out.stderr.length === 0) return "";
  if (!out.success) die(`aws ${args.slice(0, 3).join(" ")} failed${inherit ? "" : `:\n${new TextDecoder().decode(out.stderr)}`}`);
  return inherit ? "" : new TextDecoder().decode(out.stdout);
}

/** The files under `dir`, as the sync uploads them. */
function files(dir: string): number {
  let n = 0;
  for (const e of Deno.readDirSync(dir)) {
    if (e.isDirectory) n += files(`${dir}/${e.name}`);
    else if (!e.name.endsWith(".part")) n += 1;
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

async function publish(kind: string): Promise<void> {
  const dir = `out/${kind}.publish`;
  try {
    Deno.statSync(dir);
  } catch {
    die(`${kind}: no ${dir}: run \`deno task bake:${kind} --stage publish\``);
  }
  const manifest = JSON.parse(Deno.readTextFileSync(`out/${kind}/manifest.json`));
  if (!Array.isArray(manifest.shard)) die(`out/${kind}/manifest.json names no shards the set holds: what this version would serve is unknown`);
  const format: number = JSON.parse(Deno.readTextFileSync(PUBLISHED)).FORMAT[kind];
  const name = await nextName(kind, format, new Date().toISOString().slice(0, 10));
  const objects = files(dir);
  console.log(`[publish/${kind}] ${objects} objects to ${kind}/${name}`);

  // Packed already: the encoding goes with each object (publish-bucket "Upload").
  await aws([
    "s3",
    "sync",
    dir,
    `s3://${BUCKET}/${kind}/${name}`,
    "--exclude",
    "*.part",
    "--content-type",
    manifest.content_type,
    "--content-encoding",
    manifest.content_encoding,
    "--cache-control",
    "public, max-age=31536000, immutable",
  ], true);

  const stored = (await aws(["s3", "ls", `s3://${BUCKET}/${kind}/${name}/`, "--recursive"])).split("\n").filter((l) => l.trim()).length;
  if (stored !== objects) die(`${kind}/${name}: ${stored} objects stored of ${objects}; re-run the sync by hand before pointing the client at it`);

  const published = JSON.parse(Deno.readTextFileSync(PUBLISHED));
  published.SOURCE.kind[kind] = { path: name, shard: manifest.shard };
  Deno.writeTextFileSync(PUBLISHED, `${JSON.stringify(published, null, 2)}\n`);
  Deno.removeSync(dir, { recursive: true });
  console.log(`[publish/${kind}] SOURCE.kind.${kind}.path = ${name}, ${manifest.shard.length} shards`);
}

/** Four siblings all named, written as their parent, down to `top`'s length: a name is a digit a zoom. */
function folded(shards: string[], top: number): string[] {
  const keys = new Set(shards);
  for (let len = Math.max(0, ...shards.map((s) => s.length)); len > top; len--) {
    for (const parent of new Set([...keys].filter((k) => k.length === len).map((k) => k.slice(0, -1)))) {
      const children = ["0", "1", "2", "3"].map((d) => parent + d);
      if (children.every((c) => keys.has(c))) {
        children.forEach((c) => keys.delete(c));
        keys.add(parent);
      }
    }
  }
  return [...keys].sort();
}

interface Kind {
  path: string;
  shard?: string[];
}

/**
 * `SOURCE` as the client reads it (pxl-kernel io/source.rs), revalidated on
 * every boot so a publish is seen at once. Each kind's format is read off its
 * version's name, which is this script's to write and the client reads none;
 * the shards served are those every kind holds.
 */
async function publishSource(): Promise<void> {
  const { top_zoom, kind, override } = JSON.parse(Deno.readTextFileSync(PUBLISHED)).SOURCE as {
    top_zoom: number;
    kind: Record<string, Kind>;
    override: Record<string, Record<string, string>>;
  };
  const kinds = Object.entries(kind);
  const path = Object.fromEntries(kinds.map(([k, v]) => [k, v.path]));
  const format = Object.fromEntries(kinds.map(([k, v]) => {
    const n = Number(/^v(\d+)\./.exec(v.path)?.[1]);
    return Number.isInteger(n) ? [k, n] : die(`${PUBLISHED}: SOURCE.kind.${k}.path ${v.path} is no v<format>.<day>.<n>`);
  }));
  const held = kinds.flatMap(([, v]) => v.shard ? [new Set(v.shard)] : []);
  const every = [...(held[0] ?? [])].filter((s) => held.every((h) => h.has(s)));
  const shard: Record<string, { path?: Record<string, string> }> = Object.fromEntries(folded(every, top_zoom).map((k) => [k, {}]));
  for (const [key, paths] of Object.entries(override)) shard[key] = { path: paths };
  const source = { top_zoom, format, path, shard };
  const file = await Deno.makeTempFile({ suffix: ".json" });
  Deno.writeTextFileSync(file, JSON.stringify(source));
  await aws(["s3", "cp", file, `s3://${BUCKET}/${SOURCE}`, "--content-type", "application/json", "--cache-control", "no-cache"], true);
  Deno.removeSync(file);
  console.log(`[publish] ${SOURCE}: ${JSON.stringify(path)}, ${Object.keys(shard).length} keys`);
}

const asked = Deno.args;
if (asked.length === 0 || asked.some((k) => !KINDS.includes(k) && k !== SOURCE)) die(`usage: ./script/publish.ts <${[...KINDS, SOURCE].join("|")}>…`);
for (const kind of asked.filter((k) => k !== SOURCE)) await publish(kind);
const spec = await new Deno.Command("deno", { args: ["task", "spec"], stdout: "inherit", stderr: "inherit" }).output();
if (!spec.success) die("deno task spec failed");
await publishSource();
