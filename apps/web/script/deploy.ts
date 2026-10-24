#!/usr/bin/env -S deno run -A
// Publishes the client and its Worker under `PXL_BUILD_NUMBER` from CI,
// building the client first. Without a number it publishes unnumbered
// (`#<last>-<id>`, vite.config.ts `buildNumber`).
//
// `--prebuilt` publishes the `dist/` and `src/guest.paths.json` one build
// already made instead of building again: CI's gate builds before the number
// is taken, with `PXL_BUILD_NUMBER` set to STAMP, and the number is written
// over the stamp here, so what goes out is the bundle the gate checked.
//
// Everything that can fail before the Worker goes out runs before it, so a
// failure says truly what was published, and the run's last step is the
// publish itself.
//
//   ./script/deploy.ts [--prebuilt]   (from apps/web)

/** What the gate builds in place of the number it has not taken yet (the workflow's gate job sets it). */
const STAMP = "__PXL_BUILD_NUMBER__";

const number = Deno.env.get("PXL_BUILD_NUMBER");
const prebuilt = Deno.args.includes("--prebuilt");
const DIST = new URL("../dist/", import.meta.url);
const PATHS = new URL("../src/guest.paths.json", import.meta.url);

function fail(name: string, code = 1): never {
  console.error(`[deploy] ${name} failed; nothing was published`);
  Deno.exit(code);
}

async function step(name: string, cmd: string, args: string[], env: Record<string, string> = {}): Promise<void> {
  console.log(`[deploy] ${name}`);
  const status = await new Deno.Command(cmd, { args, env, stdout: "inherit", stderr: "inherit" }).output();
  if (!status.success) fail(name, status.code || 1);
}

const wrangler = (...args: string[]) => ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", ...args];

/** Every script the bundle holds, by path. */
async function scripts(dir: URL): Promise<URL[]> {
  const out: URL[] = [];
  for await (const entry of Deno.readDir(dir)) {
    const at = new URL(entry.name + (entry.isDirectory ? "/" : ""), dir);
    if (entry.isDirectory) out.push(...await scripts(at));
    else if (entry.name.endsWith(".js")) out.push(at);
  }
  return out;
}

/** The number written over the gate's stamp; a bundle with no stamp is published as it was built. */
async function stamp(): Promise<void> {
  console.log("[deploy] stamp the number");
  let found = 0;
  for (const file of await scripts(DIST)) {
    const text = await Deno.readTextFile(file);
    if (!text.includes(STAMP)) continue;
    found++;
    if (!number) {
      console.error(`[deploy] ${file.pathname} carries the gate's stamp and no PXL_BUILD_NUMBER was given`);
      fail("stamp the number");
    }
    await Deno.writeTextFile(file, text.replaceAll(STAMP, number));
  }
  if (number && found === 0) {
    console.error(`[deploy] dist/ holds no ${STAMP}, so it would not say #${number}; build it with PXL_BUILD_NUMBER=${STAMP}`);
    fail("stamp the number");
  }
}

console.log(number ? `[deploy] build #${number}` : "[deploy] no PXL_BUILD_NUMBER: publishing an unnumbered build");

if (prebuilt) {
  await stamp();
} else {
  // Lists the files behind `-dirty`, which CI once reported and nothing local reproduced.
  const status = new Deno.Command("git", { args: ["status", "--porcelain"], stdout: "piped", stderr: "null" }).outputSync();
  const unclean = status.success ? new TextDecoder().decode(status.stdout).trim() : "";
  if (unclean !== "") console.log(`[deploy] the tree is not clean, so the build id will say -dirty:\n${unclean}`);
  await step("build", "deno", ["task", "build"], number ? { PXL_BUILD_NUMBER: number } : {});
}

// The `#n` → path table for this build's panics, kept beside the build log,
// never served. Before the publish: a table for a build that then failed to
// go out is harmless, a live build without one is not.
if (number) {
  await step("keep the paths", "deno", wrangler("r2", "object", "put", `pixelet-tiles/_meta/paths/${number}.json`, "--remote", "--file", PATHS.pathname, "--content-type", "application/json"));
}

console.log("[deploy] publish");
const published = await new Deno.Command("deno", { args: wrangler("deploy"), stdout: "inherit", stderr: "inherit" }).output();
if (!published.success) {
  // wrangler can fail after the Worker went out (a route, a domain), so this
  // says no more than that the publish did not finish.
  console.error(`[deploy] publish failed; wrangler's output above says how far it got${number ? `, and _meta/paths/${number}.json is kept` : ""}`);
  Deno.exit(published.code || 1);
}

console.log(number ? `[deploy] #${number} published` : "[deploy] published, unnumbered");
