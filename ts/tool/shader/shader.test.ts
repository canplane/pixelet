// @pxl/shader/shader.test.ts

/**
 * Whether the browser's compiler takes the shaders the page hands it. Each
 * module as the renderer assembles it (pxl-render examples/assembled.rs), the
 * world's asked module among them, is compiled by Tint, the WGSL compiler a
 * browser's WebGPU runs (Dawn's, from npm:webgpu). Every pipeline the
 * renderer makes over a module, render and compute, is then made by its entry
 * points (pipeline.ts) on a device with the default limits, the floor every
 * WebGPU device binds (frame/shader/floor.rs). naga, which writes and
 * validates the shaders, takes some modules that Tint refuses, such as one
 * with a literal past f32's range, so naga's check alone is not enough.
 */

import { test } from "vitest";
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { REPO } from "@pxl/build/path.ts";
import { create, globals } from "webgpu";

import { entries, make, type Pipeline } from "./pipeline.ts";

function assembled(): {
  modules: { file: string; code: string }[];
  pipelines: Pipeline[];
} {
  const tmp = path.join(REPO, "tmp");
  fs.mkdirSync(tmp, { recursive: true });
  const dir = fs.mkdtempSync(path.join(tmp, "wgsl-"));
  try {
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
      modules: files.map((file) => ({
        file,
        code: fs.readFileSync(path.join(dir, file), "utf8"),
      })),
      pipelines: JSON.parse(
        fs.readFileSync(path.join(dir, "pipelines.json"), "utf8"),
      ),
    };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test("Tint takes every shader", { timeout: 1_200_000 }, async () => {
  const { modules, pipelines } = assembled();

  Object.assign(globalThis, globals);
  // The instance is held to the end (`void gpu` below), because an instance
  // collected while its device works ends the process.
  const gpu = create([]);
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter to compile on");
  const device = await adapter.requestDevice();
  const refused: string[] = [];
  for (const { file, code } of modules) {
    const name = file.replace(/\.wgsl$/, "");
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code });
    const errs = (await module.getCompilationInfo()).messages.filter(
      (m) => m.type === "error",
    );
    refused.push(
      ...errs.map((m) => `${file}:${m.lineNum}:${m.linePos}: ${m.message}`),
    );
    const scoped = await device.popErrorScope();
    if (scoped && errs.length === 0) {
      refused.push(`${file}: ${scoped.message.split("\n")[0]}`);
    }
    console.log(
      `${file}: ${errs.length > 0 || scoped ? "refused" : "compiled"}`,
    );
    // A module that does not compile would fail its pipelines too, so it is
    // reported once and its pipelines are not made.
    if (errs.length > 0 || scoped) continue;
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
