// @pxl/web/shader.test.ts

/**
 * Whether the browser's compiler takes the shaders the page hands it: each
 * module as the renderer assembles it (pxl-render examples/assembled.rs), the
 * world's asked one among them, compiled by Tint, the WGSL compiler a
 * browser's WebGPU runs (Dawn's, npm:webgpu), and a compute module's pipeline
 * made with the layout it declares on a device of the default limits, the floor
 * every WebGPU device binds (frame/shader/floor.rs). naga, which writes and
 * validates them, takes modules Tint refuses (a literal past f32's range).
 */

import { test } from "vitest";
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { REPO } from "@pxl/build/paths.ts";
import { create, globals } from "webgpu";

/** Each hand module, assembled here by the renderer's own example. */
function assembled(): string[] {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "pxl-wgsl-"));
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "pxl-render", "--example", "assembled", "--", dir],
    { cwd: path.join(REPO, "rust"), stdio: "inherit" },
  );
  const files = fs.readdirSync(dir).sort();
  if (!files.includes("world_asked.wgsl") || files.length < 2) {
    throw new Error(`no hand module under ${dir}`);
  }
  return files.map((f) => path.join(dir, f));
}

test("Tint takes every shader", { timeout: 1_200_000 }, async () => {
  const modules = assembled();

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
    const code = fs.readFileSync(at, "utf8");
    // A render module is compiled alone: its pipelines are the renderer's records.
    const entry = code.match(/@compute[^]*?\bfn\s+(\w+)\s*\(/)?.[1];
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter(
      (m) => m.type === "error",
    );
    refused.push(
      ...errors.map((m) => `${file}:${m.lineNum}:${m.linePos}: ${m.message}`),
    );
    // Its errors, as the module's past its compile, the scope's.
    if (entry) {
      device.createComputePipeline({
        layout: "auto",
        compute: { module, entryPoint: entry },
      });
    }
    // A module that does not compile fails its pipeline too: said once.
    const scoped = await device.popErrorScope();
    if (scoped && errors.length === 0) {
      refused.push(`${file}: ${scoped.message.split("\n")[0]}`);
    }
    console.log(
      `${file}: ${errors.length > 0 || scoped ? "refused" : entry ? "pipeline made" : "compiled"}`,
    );
  }
  device.destroy();
  void gpu;
  if (refused.length > 0) {
    throw new Error(`refused by Tint:\n${refused.join("\n")}`);
  }
});
