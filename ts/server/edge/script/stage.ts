#!/usr/bin/env -S deno run -A
// Builds the page (ts/app/web `build`) and stages its bundle beside the
// Worker, at dist/, which wrangler.jsonc `assets` serves: a fresh copy every
// time, so what is checked or deployed is never a bundle left from before.
// A build number in the environment (PXL_BUILD_NUMBER) is the build's.
//
//   ./script/stage.ts   (from ts/server/edge)

import { WORKSPACE } from "@pxl/client/script/import-map.ts";

const WEB = `${WORKSPACE}/app/web`;
const STAGED = new URL("../dist/", import.meta.url).pathname;

const built = await new Deno.Command("deno", { args: ["task", "--cwd", WEB, "build"], stdout: "inherit", stderr: "inherit" }).output();
if (!built.success) Deno.exit(built.code || 1);

await Deno.remove(STAGED, { recursive: true }).catch((e) => {
  if (!(e instanceof Deno.errors.NotFound)) throw e;
});
const copied = await new Deno.Command("cp", { args: ["-R", `${WEB}/dist/`, STAGED] }).output();
if (!copied.success) Deno.exit(copied.code || 1);
console.log(`[stage] ${WEB}/dist -> ${STAGED}`);
