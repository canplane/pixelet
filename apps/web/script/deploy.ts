#!/usr/bin/env -S deno run -A
// Builds, checks and publishes the client.
//
// The number this ships under is handed in as `PXL_BUILD_NUMBER`, by the gate
// job that took it (script/build-counter.ts). This does not take one: a build
// lab has one place that hands numbers out, and since there is a CI building
// main, that place is the CI.
//
// Run by hand with nothing in the environment, it publishes an unnumbered
// build — `b<last>+` in the about panel, which says "after b<last>, not one of
// them". That is the honest thing for a deploy nobody counted, and it is the
// fallback rather than the way.
//
// Usage (from apps/web):
//   ./script/deploy.ts

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

// What the build id is about to call this tree. `git status --porcelain` from
// here is exactly what vite.config.ts's build_id() asks, and it answers with
// paths — so a build that would otherwise only say `-dirty` says which files
// made it so. Worth printing on every deploy and not only when one is being
// chased: a CI build reports `-dirty` today and nothing local reproduces it,
// which is the state this line exists to end.
const status = new Deno.Command("git", { args: ["status", "--porcelain"], stdout: "piped", stderr: "null" }).outputSync();
const unclean = status.success ? new TextDecoder().decode(status.stdout).trim() : "";
if (unclean !== "") console.log(`[deploy] the tree is not clean, so the build id will say -dirty:\n${unclean}`);

await step("build", "deno", ["task", "build"], number ? { PXL_BUILD_NUMBER: number } : {});
await step("check:bundle", "deno", ["task", "check:bundle"]);
await step("publish", "deno", wrangler("deploy"));

// This checkout's copy of the last number it saw published, so a dev server
// can say which official build it follows without wrangler or a login. The
// count itself is the bucket's; this is only a cache, and gitignored.
if (number) await Deno.writeTextFile(new URL("../.build_number", import.meta.url), `${number}\n`);
console.log(number ? `[deploy] #${number} published` : "[deploy] published, unnumbered");
