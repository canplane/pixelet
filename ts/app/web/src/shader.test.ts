// @pxl/web/shader.test.ts

/**
 * Whether the browser's compiler takes the generated shaders the page hands
 * it: each module pxl-tool checks in beside the renderer's shaders
 * (pxl-tool shader.rs), compiled by Tint, the WGSL compiler a browser's
 * WebGPU runs (Dawn's, npm:webgpu), and its compute pipeline made with the
 * layout it declares on a device of the default limits, the floor every
 * WebGPU device binds (frame/shader/floor.rs). naga, which writes them, takes
 * modules Tint refuses (a literal past f32's range, at e84ddd56e).
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { REPO } from "@pxl/build/import-map.ts";
import { create, globals } from "webgpu";

const SHADERS = path.join(REPO, "rust/client/render/src/frame/shader");

Deno.test("Tint takes every generated shader", async () => {
  const modules = fs.readdirSync(SHADERS).filter((f) => f.endsWith(".gen.wgsl")).sort();
  if (modules.length === 0) throw new Error(`no generated shader under ${SHADERS}`);

  Object.assign(globalThis, globals);
  // Held to the end (`void gpu` below): an instance collected while its
  // device works ends the process.
  const gpu = create([]);
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter to compile on");
  const device = await adapter.requestDevice();
  const refused: string[] = [];
  for (const file of modules) {
    const code = fs.readFileSync(path.join(SHADERS, file), "utf8");
    const entry = code.match(/@compute[^]*?\bfn\s+(\w+)\s*\(/)?.[1];
    if (!entry) {
      refused.push(`${file}: no @compute entry`);
      continue;
    }
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
    refused.push(...errors.map((m) => `${file}:${m.lineNum}:${m.linePos}: ${m.message}`));
    // Its errors, as the module's past its compile, the scope's.
    device.createComputePipeline({ layout: "auto", compute: { module, entryPoint: entry } });
    // A module that does not compile fails its pipeline too: said once.
    const scoped = await device.popErrorScope();
    if (scoped && errors.length === 0) refused.push(`${file}: ${scoped.message.split("\n")[0]}`);
  }
  device.destroy();
  void gpu;
  if (refused.length > 0) throw new Error(`refused by Tint:\n${refused.join("\n")}`);
});
