#!/usr/bin/env -S deno run -A
// What Tint, the WGSL compiler a browser's WebGPU runs (Dawn's, npm:webgpu),
// refuses of the modules on its input: a JSON array of { entry, file, code },
// as pxl-tool's bin/shader_wgsl.rs gives them. Each module's compile errors
// and its compute pipeline's, made with the layout it declares on a device of
// the default limits, the floor every WebGPU device binds
// (frame/shader/floor.rs), are printed a line each; it exits 1 if any.
// TypeScript because Dawn is an npm library.
//
//   deno run -A script/tint.ts < modules.json

import { create, globals } from "webgpu";

interface Module {
  entry: string;
  file: string;
  code: string;
}

const modules = JSON.parse(await new Response(Deno.stdin.readable).text()) as Module[];

Object.assign(globalThis, globals);
// Held to the end (`void gpu` below): an instance collected while its device
// works ends the process.
const gpu = create([]);
const adapter = await gpu.requestAdapter();
if (!adapter) {
  console.log("no WebGPU adapter to compile on");
  Deno.exit(1);
}
const device = await adapter.requestDevice();
const out: string[] = [];
for (const { entry, file, code } of modules) {
  device.pushErrorScope("validation");
  const module = device.createShaderModule({ code });
  const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
  out.push(...errors.map((m) => `${file}:${m.lineNum}:${m.linePos}: ${m.message}`));
  // Its errors, as the module's past its compile, the scope's.
  device.createComputePipeline({ layout: "auto", compute: { module, entryPoint: entry } });
  // A module that does not compile fails its pipeline too: said once.
  const scoped = await device.popErrorScope();
  if (scoped && errors.length === 0) out.push(`${file}: ${scoped.message.split("\n")[0]}`);
}
device.destroy();
void gpu;
for (const line of out) console.log(line);
Deno.exit(out.length > 0 ? 1 : 0);
