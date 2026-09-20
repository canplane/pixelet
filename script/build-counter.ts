#!/usr/bin/env -S deno run -A
// Takes the next official build number off the counter and prints it.
//
// The count is a build lab's: one place hands numbers out, every build main
// gets one, and some of those are shipped. That place is CI — the gate job,
// which runs on every push to main, whether or not the build turns out to be
// shippable. A number that a red gate consumed is a number nobody sees, which
// is what a build lab's count does too.
//
// The counter is one object in the bucket rather than anything in git, where
// no squash, rebase or fresh clone can renumber it, and rather than the
// workflow's own run number, which restarts at 1 if the workflow is ever
// recreated. It is never reset.
//
// Counter and number are not the same word. The counter is the state that
// keeps going up, as TeamCity's `build.counter` is; the number is what one
// build carries, as `BUILD_NUMBER` is everywhere. The object holds the first
// and `PXL_BUILD_NUMBER` carries the second.
//
// The take is not atomic: read, add one, write. What keeps two runs from
// taking the same number is that only one holds the gate's concurrency group
// at a time (.github/workflows/check-and-deploy.yml). A hand deploy is outside
// that group and does not take a number at all — it publishes unnumbered.
//
// Usage (from the repo root):
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
  // Not "so this is build 1". A count that restarts is worse than a build
  // that does not happen, and the only way to start one is to say so:
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
  die(`could not write the counter back; b${next} was not taken and the next build would reuse it`);
}

// The number alone on stdout: the workflow reads it into a job output.
console.log(String(next));
