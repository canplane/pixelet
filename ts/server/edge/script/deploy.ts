#!/usr/bin/env -S deno run -A
// Builds and publishes the client and its Worker under `PXL_BUILD_NUMBER`
// from CI. Without a number it publishes unnumbered (`#<last>-<id>`,
// ts/app/web/vite.config.ts `buildNumber`).
//
// Everything that can fail before the Worker goes out runs before it, so a
// failure says truly what was published, and the last step is the publish
// itself.
//
//   ./script/deploy.ts   (from ts/server/edge)

import { WORKSPACE } from "@pxl/client/script/import-map.ts";

const number = Deno.env.get("PXL_BUILD_NUMBER");
/** The panics' `#n` → path table the page's build wrote beside its guest. */
const PATHS = `${WORKSPACE}/app/web/src/guest.paths.json`;
const BUILT = new URL("../dist/index.html", import.meta.url);
/** The custom domain the Worker answers on (wrangler.jsonc `routes`). */
const SITE = "https://plei.me";

/** What a page loads: its hashed scripts and styles, which name the build. */
const loads = (html: string) => [...html.matchAll(/(?:src|href)="(\/assets\/[^"]+)"/g)].map((m) => m[1]).sort().join(" ");

/** Whether the site serves this build: what wrangler's exit cannot say once
 * it failed after the Worker went out (a route, a domain). */
async function live(): Promise<boolean> {
  try {
    const res = await fetch(`${SITE}/?_deploy=${crypto.randomUUID()}`);
    const built = loads(Deno.readTextFileSync(BUILT));
    return res.ok && built !== "" && loads(await res.text()) === built;
  } catch {
    return false;
  }
}

/** Said to CI's next jobs (`published`), where the run has an output file. */
function landed(yes: boolean): void {
  const out = Deno.env.get("GITHUB_OUTPUT");
  if (out) Deno.writeTextFileSync(out, `published=${yes}\n`, { append: true });
}

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

await step("build", "deno", ["run", "-A", new URL("stage.ts", import.meta.url).pathname], number ? { PXL_BUILD_NUMBER: number } : {});

// The `#n` → path table for this build's panics, kept beside the build log,
// never served. Before the publish: a table for a build that then failed to
// go out is harmless, a live build without one is not.
if (number) {
  await step("keep the paths", "deno", wrangler("r2", "object", "put", `pixelet-tiles/_meta/paths/${number}.json`, "--remote", "--file", PATHS, "--content-type", "application/json"));
}

console.log("[deploy] publish");
const published = await new Deno.Command("deno", { args: wrangler("deploy"), stdout: "inherit", stderr: "inherit" }).output();
if (!published.success) {
  // wrangler can fail after the Worker went out (a route, a domain), so what
  // landed is read off the site, not off its exit.
  const yes = await live();
  landed(yes);
  console.error(`[deploy] publish failed, and ${SITE} serves ${yes ? "this build all the same" : "an earlier one"}${number ? `; _meta/paths/${number}.json is kept` : ""}`);
  Deno.exit(published.code || 1);
}

landed(true);
console.log(number ? `[deploy] #${number} published` : "[deploy] published, unnumbered");
