// @pxl/shader/vite.config.ts

/**
 * The shader probe's dev server (probe.html): each module as the renderer
 * assembles it (pxl-render examples/assembled.rs) at `/__assembled`, the
 * experiments' WGSL read from the repository, and a device's console printed
 * here (@pxl/build/console.ts). `pnpm probe`, then open /probe.html on the
 * device, over a tunnel where it is not this machine: WebGPU needs a secure
 * context.
 */

import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import { defineConfig, type Plugin } from "vite";

import { CONSOLE_PATH, consoleHandler } from "@pxl/build/console.ts";
import { REPO } from "@pxl/build/paths.ts";

/** The renderer's modules by name, assembled once as the server first asks. */
function assembled(): Record<string, string> {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "pxl-probe-"));
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "pxl-render", "--example", "assembled", "--", dir],
    { cwd: path.join(REPO, "rust"), stdio: "inherit" },
  );
  return Object.fromEntries(
    fs
      .readdirSync(dir)
      .filter((f) => f.endsWith(".wgsl"))
      .map((f) => [f.replace(/\.wgsl$/, ""), fs.readFileSync(path.join(dir, f), "utf8")]),
  );
}

function probeServer(): Plugin {
  let modules: string | undefined;
  return {
    name: "pxl-shader-probe",
    configureServer(server) {
      server.middlewares.use(CONSOLE_PATH, consoleHandler("./.dev-console.log"));
      server.middlewares.use("/__assembled", (_req, res) => {
        try {
          modules ??= JSON.stringify(assembled());
          res.setHeader("content-type", "application/json");
          res.end(modules);
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
    },
  };
}

export default defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [probeServer()],
  server: {
    host: true,
    allowedHosts: [".trycloudflare.com"],
    fs: { allow: [REPO] },
  },
});
