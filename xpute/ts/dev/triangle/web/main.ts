// @xpute/triangle/web/main.ts

/**
 * The page starts the core in its worker, opens a surface on it, lends the core
 * the canvas under the surface's handle, and tells it the canvas's size. The
 * core draws every frame from then on.
 */

import { Command } from "@xpute/gpu/abi.spec.ts";
import { gpu_protocol } from "@xpute/gpu/gpu.ts";
import { gpu_connect } from "@xpute/gpu/web.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { QUIET, start } from "@xpute/runtime";
import { GPU_MAJOR, manifest } from "./core.ts";

const canvas = document.querySelector<HTMLCanvasElement>("#surface")!;
const worker = new Worker(new URL("./core.worker.ts", import.meta.url), {
  type: "module",
});

const { port1, port2 } = new MessageChannel();
const binder = gpu_connect(port1, (state) => console.log("gpu:", state));
worker.postMessage({ gpu: port2 }, [port2]);

const link = await start(manifest, QUIET, () => worker);
link.resume();

const gpu = gpu_protocol(link, GPU_MAJOR);
const surface = Math.max(0, await gpu.request(Command.GPU_SURFACE_OPEN));
binder.bind(surface, canvas);

const dpr = devicePixelRatio;
gpu.send(
  Command.GPU_SURFACE_RESIZE,
  surface,
  round(canvas.clientWidth * dpr),
  round(canvas.clientHeight * dpr),
  dpr,
);
