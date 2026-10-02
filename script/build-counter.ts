#!/usr/bin/env -S deno run -A
// Takes the next official build number off the counter and prints it. The
// counter is one object in the bucket, so no history rewrite renumbers it,
// and it is never reset. The take is read-add-write; the gate's concurrency
// group (.github/workflows/check-and-deploy.yml) keeps two runs apart.
//
//   ./script/build-counter.ts

const BUCKET = "pixelet-tiles";
const KEY = "_meta/build-counter";

const wrangler = (...args: string[]) => ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", ...args];

async function run(args: string[]): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const out = await new Deno.Command("deno", { args, stdout: "piped", stderr: "piped" }).output();
  return { ok: out.success, stdout: new TextDecoder().decode(out.stdout), stderr: new TextDecoder().decode(out.stderr) };
}

function die(message: string): never {
  console.error(`[build-counter] ${message}`);
  Deno.exit(1);
}

const got = await run(wrangler("r2", "object", "get", `${BUCKET}/${KEY}`, "--remote", "--pipe"));
if (!got.ok) {
  console.error(got.stderr);
  // A count that restarts is worse than no build; start one explicitly:
  //   echo N | wrangler r2 object put pixelet-tiles/_meta/build-counter --remote --pipe --content-type text/plain
  die(`no counter at ${KEY}; no number was taken`);
}
const current = Number(got.stdout.trim());
if (!Number.isSafeInteger(current) || current < 0) {
  die(`the counter holds ${JSON.stringify(got.stdout.slice(0, 40))}, not a count; no number was taken`);
}

const next = current + 1;
const file = await Deno.makeTempFile();
await Deno.writeTextFile(file, `${next}\n`);
const put = await run(wrangler("r2", "object", "put", `${BUCKET}/${KEY}`, "--remote", "--file", file, "--content-type", "text/plain"));
await Deno.remove(file);
if (!put.ok) {
  console.error(put.stderr);
  die(`could not write the counter back; #${next} was not taken and the next build would reuse it`);
}

console.log(String(next));
