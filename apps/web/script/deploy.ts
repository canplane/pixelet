#!/usr/bin/env -S deno run -A
// Builds, checks and publishes the client under `PXL_BUILD_NUMBER` from CI.
// Without one it publishes unnumbered (`b<last>+`).
//
//   ./script/deploy.ts   (from apps/web)

const number = Deno.env.get("PXL_BUILD_NUMBER");

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
await step("publish", "deno", wrangler("deploy"));
// The `#n` → path table for this build's panics, kept beside the build log, never served.
if (number) {
  const paths = new URL("../src/guest.paths.json", import.meta.url);
  await step("keep the paths", "deno", wrangler("r2", "object", "put", `pixelet-tiles/_meta/paths/${number}.json`, "--remote", "--file", paths.pathname, "--content-type", "application/json"));
}

console.log(number ? `[deploy] #${number} published` : "[deploy] published, unnumbered");
