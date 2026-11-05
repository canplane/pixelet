// @pxl/shader/shader.test.ts

/**
 * Whether the browser's compiler takes the shaders the page hands it: each
 * module as the renderer assembles it (pxl-render examples/assembled.rs), the
 * world's asked one among them, compiled by Tint, the WGSL compiler a
 * browser's WebGPU runs (Dawn's, npm:webgpu), and every pipeline the renderer
 * makes over it, render and compute, made by its entry points (pipeline.ts)
 * on a device of the default limits, the floor every WebGPU device binds
 * (frame/shader/floor.rs). naga, which writes and validates them, takes
 * modules Tint refuses (a literal past f32's range).
 */

import { test } from "vitest";
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { REPO } from "@pxl/build/paths.ts";
import { create, globals } from "webgpu";

import { entries, make, type Pipeline } from "./pipeline.ts";

/** Each hand module, assembled here by the renderer's own example, by its
 * path, and the pipelines the renderer makes over them. */
function assembled(): { modules: string[]; pipelines: Pipeline[] } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "pxl-wgsl-"));
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "pxl-render", "--example", "assembled", "--", dir],
    { cwd: path.join(REPO, "rust"), stdio: "inherit" },
  );
  const files = fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".wgsl"))
    .sort();
  if (!files.includes("world_asked.wgsl") || files.length < 2) {
    throw new Error(`no hand module under ${dir}`);
  }
  return {
    modules: files.map((f) => path.join(dir, f)),
    pipelines: JSON.parse(
      fs.readFileSync(path.join(dir, "pipelines.json"), "utf8"),
    ),
  };
}

test("Tint takes every shader", { timeout: 1_200_000 }, async () => {
  const { modules, pipelines } = assembled();

  Object.assign(globalThis, globals);
  // Held to the end (`void gpu` below): an instance collected while its
  // device works ends the process.
  const gpu = create([]);
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter to compile on");
  const device = await adapter.requestDevice();
  const refused: string[] = [];
  for (const at of modules) {
    const file = path.basename(at);
    const name = file.replace(/\.wgsl$/, "");
    const code = fs.readFileSync(at, "utf8");
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter(
      (m) => m.type === "error",
    );
    refused.push(
      ...errors.map((m) => `${file}:${m.lineNum}:${m.linePos}: ${m.message}`),
    );
    const scoped = await device.popErrorScope();
    if (scoped && errors.length === 0) {
      refused.push(`${file}: ${scoped.message.split("\n")[0]}`);
    }
    console.log(
      `${file}: ${errors.length > 0 || scoped ? "refused" : "compiled"}`,
    );
    // A module that does not compile fails its pipelines too: said once.
    if (errors.length > 0 || scoped) continue;
    for (const p of pipelines.filter((p) => p.module === name)) {
      device.pushErrorScope("validation");
      const said = await make(device, module, code, p).then(
        () => null,
        (e: Error) => e.message.split("\n")[0],
      );
      const invalid = await device.popErrorScope();
      const errs = [said, invalid?.message.split("\n")[0]].filter(Boolean);
      refused.push(...errs.map((m) => `${file} ${entries(p)}: ${m}`));
      console.log(
        `${file} ${entries(p)}: ${errs.length > 0 ? "refused" : "pipeline made"}`,
      );
    }
  }
  device.destroy();
  void gpu;
  if (refused.length > 0) {
    throw new Error(`refused by Tint:\n${refused.join("\n")}`);
  }
});
