// @pxl/web/../script/deploy.ts
//
// Builds, checks and publishes the client under the next build number.
//
// The number counts official builds the way a build lab does: one place
// hands them out, and a local build never takes one. With no CI, that place
// is this deploy, so the counter lives outside the repository — one object
// in the bucket — where no squash, rebase or fresh clone can renumber it.
// When a CI builds main, the increment moves there and the count carries on
// from wherever this left it; it is never reset.
//
// The next number is read before the build, since the bundle carries it, and
// written only after the Worker is published: a deploy that fails anywhere
// leaves the counter where it was and the number is taken by the next one.
// Two deploys at once would take the same number — this has one deployer.
//
// The number is also cached, gitignored, in .build_number, so a dev server
// or a local build can say which official build it follows (`b123+`)
// without wrangler, a login or the network.
//
// Usage (from apps/web):
//   deno run -A script/deploy.ts

const BUCKET = "pixelet-tiles";
const COUNTER_KEY = "meta/build-number";
const CACHE = new URL("../.build_number", import.meta.url);

async function run(cmd: string, args: string[], env: Record<string, string> = {}): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const out = await new Deno.Command(cmd, { args, env, stdout: "piped", stderr: "piped" }).output();
  return { ok: out.success, stdout: new TextDecoder().decode(out.stdout), stderr: new TextDecoder().decode(out.stderr) };
}

async function step(name: string, cmd: string, args: string[], env: Record<string, string> = {}): Promise<void> {
  console.log(`[deploy] ${name}`);
  const status = await new Deno.Command(cmd, { args, env, stdout: "inherit", stderr: "inherit" }).output();
  if (!status.success) {
    console.error(`[deploy] ${name} failed; the build number was not taken`);
    Deno.exit(status.code || 1);
  }
}

const wrangler = (...args: string[]) => ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", ...args];

async function read_counter(): Promise<number> {
  const got = await run("deno", wrangler("r2", "object", "get", `${BUCKET}/${COUNTER_KEY}`, "--remote", "--pipe"));
  if (!got.ok) {
    // The first deploy finds no counter. Anything else — no login, no
    // network — stops here rather than restarting the count at 1.
    if (got.stderr.includes("The specified key does not exist")) return 0;
    console.error(got.stderr);
    console.error("[deploy] could not read the build counter; nothing was built or published");
    Deno.exit(1);
  }
  const n = Number(got.stdout.trim());
  if (!Number.isSafeInteger(n) || n < 0) {
    console.error(`[deploy] the build counter holds ${JSON.stringify(got.stdout.slice(0, 40))}, not a count; nothing was built or published`);
    Deno.exit(1);
  }
  return n;
}

const next = (await read_counter()) + 1;
const env = { PXL_BUILD_NUMBER: String(next) };
console.log(`[deploy] build b${next}`);

await step("build", "deno", ["task", "build"], env);
await step("check:bundle", "deno", ["task", "check:bundle"]);
await step("publish", "deno", wrangler("deploy"));

const file = await Deno.makeTempFile();
await Deno.writeTextFile(file, `${next}\n`);
const put = await run("deno", wrangler("r2", "object", "put", `${BUCKET}/${COUNTER_KEY}`, "--remote", "--file", file, "--content-type", "text/plain"));
await Deno.remove(file);
if (!put.ok) {
  console.error(put.stderr);
  console.error(`[deploy] b${next} is published, but the counter was not written: set ${COUNTER_KEY} to ${next} before the next deploy, or it will reuse the number`);
  Deno.exit(1);
}
await Deno.writeTextFile(CACHE, `${next}\n`);
console.log(`[deploy] b${next} published and counted`);
