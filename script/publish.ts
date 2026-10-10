#!/usr/bin/env -S deno run -A
// Uploads each kind's `out/<kind>.publish/` as a new version and points
// published.json at it. The name is `v<format>.<UTC day>.<n>`: the format is
// published.json's `FORMAT`, set by hand; n is past every version of that
// format and day the bucket holds, so no version is written over. A kind
// keeps the new version, the one it replaces and the one deploy reads;
// older ones are removed. Uses the
// AWS CLI's `r2` profile and `$R2`, the bucket's S3 endpoint (publish-bucket).
// Deploying is `deno task deploy`, after.
//
//   ./script/publish.ts <terrain|place|division>…

const BUCKET = "pixelet-tiles";
const PUBLISHED = "spec/spec/bake/published.json";
const KINDS = ["terrain", "place", "division"];

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

/** The version `deploy`'s tree points `kind` at; with none to be read, nothing is removed. */
function live(kind: string): string | null {
  new Deno.Command("git", { args: ["fetch", "-q", "origin", "deploy"] }).outputSync();
  const out = new Deno.Command("git", { args: ["show", `origin/deploy:${PUBLISHED}`], stdout: "piped", stderr: "null" }).outputSync();
  return out.success ? JSON.parse(new TextDecoder().decode(out.stdout)).PUBLISHED_VERSION[kind] ?? null : null;
}

async function publish(kind: string): Promise<void> {
  const dir = `out/${kind}.publish`;
  try {
    Deno.statSync(dir);
  } catch {
    die(`${kind}: no ${dir}: run \`deno task bake:${kind} --stage publish\``);
  }
  const manifest = JSON.parse(Deno.readTextFileSync(`out/${kind}/manifest.json`));
  const published = JSON.parse(Deno.readTextFileSync(PUBLISHED));
  const format: number = published.FORMAT[kind];
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

  // One line of the file, so the rest keeps its layout.
  const text = Deno.readTextFileSync(PUBLISHED);
  const block = text.indexOf('"PUBLISHED_VERSION"');
  const line = new RegExp(`("${kind}": )"[^"]*"`);
  const at = text.slice(block).search(line);
  if (block < 0 || at < 0) die(`${PUBLISHED}: no PUBLISHED_VERSION.${kind}`);
  Deno.writeTextFileSync(PUBLISHED, text.slice(0, block) + text.slice(block).replace(line, `$1"${name}"`));
  Deno.removeSync(dir, { recursive: true });
  console.log(`[publish/${kind}] PUBLISHED_VERSION.${kind} = ${name}`);

  // Kept: the new version, the one it replaces, and the one deploy's tree
  // names, which the live site reads until the client goes out — a publish
  // not yet deployed leaves the last two apart. A delete costs no operation.
  const deployed = live(kind);
  const keep = [name, published.PUBLISHED_VERSION[kind], deployed];
  for (const old of deployed ? (await versions(kind)).filter((v) => !keep.includes(v)) : []) {
    await aws(["s3", "rm", `s3://${BUCKET}/${kind}/${old}/`, "--recursive", "--only-show-errors"], true);
    console.log(`[publish/${kind}] removed ${kind}/${old}`);
  }
}

const asked = Deno.args;
if (asked.length === 0 || asked.some((k) => !KINDS.includes(k))) die(`usage: ./script/publish.ts <${KINDS.join("|")}>…`);
for (const kind of asked) await publish(kind);
const spec = await new Deno.Command("deno", { args: ["task", "spec"], stdout: "inherit", stderr: "inherit" }).output();
if (!spec.success) die("deno task spec failed");
