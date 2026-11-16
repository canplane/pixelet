// @pxl/shader/vite.config.ts

/**
 * The shader probe's dev server (probe.html). It serves each module as the
 * renderer assembles it, and the pipelines the renderer makes over them
 * (pxl-render examples/assembled.rs), at `/__assembled`, and prints a device's
 * console here (console.ts). Run `pnpm probe`, then open
 * /probe.html on the device. A device other than this machine must reach it
 * over a tunnel, because WebGPU needs a secure context.
 */

import { execFile } from "node:child_process";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { promisify } from "node:util";

import { defineConfig, type Plugin } from "vite";

import { REPO } from "@pxl/build/path.ts";

import { CONSOLE_PATH, console_handler } from "./console.ts";

/** The modules are assembled again at each ask: cargo rebuilds the example
 * after a shader is edited, and reuses it otherwise. */
async function assembled(): Promise<{
  modules: Record<string, string>;
  pipelines: unknown[];
}> {
  const tmp = path.join(REPO, "tmp");
  await fs.mkdir(tmp, { recursive: true });
  const dir = await fs.mkdtemp(path.join(tmp, "probe-"));
  try {
    await promisify(execFile)(
      "cargo",
      ["run", "-q", "-p", "pxl-render", "--example", "assembled", "--", dir],
      { cwd: path.join(REPO, "rust") },
    );
    const files = (await fs.readdir(dir)).filter((f) => f.endsWith(".wgsl"));
    return {
      modules: Object.fromEntries(
        await Promise.all(
          files.map(async (f) => [
            f.replace(/\.wgsl$/, ""),
            await fs.readFile(path.join(dir, f), "utf8"),
          ]),
        ),
      ),
      pipelines: JSON.parse(
        await fs.readFile(path.join(dir, "pipelines.json"), "utf8"),
      ),
    };
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

function probe_server(): Plugin {
  return {
    name: "pxl-shader-probe",
    configureServer(server) {
      server.middlewares.use(
        CONSOLE_PATH,
        console_handler(path.join(REPO, "ts/tool/shader/.dev-console.log")),
      );
      server.middlewares.use("/__assembled", (_req, res) => {
        assembled().then(
          (assembly) => {
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify(assembly));
          },
          (e: unknown) => {
            const err = e as { stderr?: string };
            res.statusCode = 500;
            res.end(err.stderr || String(e));
          },
        );
      });
    },
  };
}

export default defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [probe_server()],
  server: {
    host: true,
    allowedHosts: [".trycloudflare.com"],
    fs: { allow: [REPO] },
  },
});
