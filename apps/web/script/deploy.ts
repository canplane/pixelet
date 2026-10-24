#!/usr/bin/env -S deno run -A
// Builds and publishes the client and its Worker under `PXL_BUILD_NUMBER`
// from CI. Without a number it publishes unnumbered (`#<last>-<id>`,
// vite.config.ts `buildNumber`).
//
// Everything that can fail before the Worker goes out runs before it, so a
// failure says truly what was published, and the last step is the publish
// itself.
//
//   ./script/deploy.ts   (from apps/web)

const number = Deno.env.get("PXL_BUILD_NUMBER");
const PATHS = new URL("../src/guest.paths.json", import.meta.url);

async function step(name: string, cmd: string, args: string[], env: Record<string, string> = {}): Promise<void> {
  console.log(`[deploy] ${name}`);
  const status = await new Deno.Command(cmd, { args, env, stdout: "inherit", stderr: "inherit" }).output();
  if (!status.success) {
    console.error(`[deploy] ${name} failed; nothing was published`);
    Deno.exit(status.code || 1);
  }
}

const wrangler = (...args: string[]) => ["run", "-A", "--node-modules-dir=auto", "npm:wrangler", ...args];

console.log(number ? `[deploy] build #${number}` : "[deploy] no PXL_BUILD_NUMBER: publishing an unnumbered build");

// Lists the files behind `-dirty`, which CI once reported and nothing local reproduced.
const status = new Deno.Command("git", { args: ["status", "--porcelain"], stdout: "piped", stderr: "null" }).outputSync();
const unclean = status.success ? new TextDecoder().decode(status.stdout).trim() : "";
if (unclean !== "") console.log(`[deploy] the tree is not clean, so the build id will say -dirty:\n${unclean}`);

await step("build", "deno", ["task", "build"], number ? { PXL_BUILD_NUMBER: number } : {});

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
