// @pxl/client/runtime/worker.ts

/**
 * A page's core and the host's link to it run in a worker of their own, so
 * that the core holds up none of the UI's events. The page sends, in one
 * message, its kept entries, the core's module, the UI's port, and the named
 * handles and prefixes of what the UI serves; the worker instantiates the
 * module with a link laid over it (@xpute/web-runtime `instantiate`), mounts
 * the objects the host serves, the core's files and GPU among them, and the
 * UI's stand-ins, enters the core, and serves the link to the UI across the
 * port (@xpute/link remote.ts), the canvases the UI gives going to the GPU.
 * The files are here, so the source is declared here too.
 */

import type { GpuState } from "@xpute/gpu-link";
import { type Side, serve, wait } from "@xpute/link";
import { hook, serve_clock, serve_stop } from "@xpute/web-host";
import {
  forward_console,
  type Instantiate,
  instantiate,
  type InstantiateError,
  type Log,
  type Stopped,
  type Trap,
} from "@xpute/web-runtime";
import { serve_vsync, WebGpu } from "@xpute/gpu-web-host";
import { device } from "@/abi/gpu.spec.ts";
import { LATER } from "@/abi/later.ts";
import { mem } from "@/abi/manifest.spec.ts";
import { LIMIT } from "@/abi/perf.spec.ts";
import { set_online } from "@/service/origin/online.ts";
import { serve_files } from "@/service/sys/fs/file.ts";
import { serve_objects } from "@/service/sys/fs/object.ts";
import {
  type Declaration,
  declare_source,
  type Source,
  source_decl,
  source_state,
  watch_source,
} from "@/service/sys/fs/source.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { serve_text } from "@/service/core/text.ts";
import * as online from "@/service/platform/online.ts";

/** What the page sends the worker: the core's instantiation
 * (@xpute/web-runtime `Instantiate`), with the page's kept entries, which a
 * worker cannot read. */
export type ToWorker = Instantiate & { entries: [string, string][] };

/** What the worker sends the page: the source's state, a kept key it wrote,
 * the GPU's state, the error the core's instantiation came to, the core's
 * report before it stops, its trap, and a line of the worker's console. */
export type ToPage =
  | { kind: "source"; state: Source; decl: Declaration | null }
  | { kind: "keep"; key: string; val: string }
  | { kind: "gpu"; state: GpuState }
  | Stopped
  | InstantiateError
  | Trap
  | Log;

const tell = (m: ToPage) => postMessage(m);

// What the worker writes to its console goes to the page's lines too, where a
// tool driving the page reads them (service/platform/log.ts).
forward_console(tell);

onmessage = ({ data: m }: MessageEvent<ToWorker>) => {
  keep(m.entries);
  void start(m);
};

/** A copy of the page's kept entries, each write sent back to the page. */
function keep(entries: [string, string][]): void {
  const store = new Map(entries);
  set_storage({
    get: (key) => store.get(key) ?? null,
    set: (key, val) => {
      store.set(key, val);
      tell({ kind: "keep", key, val });
    },
  });
}

async function start(m: Instantiate): Promise<void> {
  online.watch(set_online);
  const link = await instantiate(m.module, m.init, mem.sz, tell);
  if (link === null) return;
  serve_stop(link, (stop) => tell({ kind: "stopped", stop }));
  hook(link);
  serve_clock(link);
  serve_vsync(link);
  serve_text(link);
  serve_files(link);
  serve_objects(link);
  // The page's host stages its writes in the chunks a device's does
  // (spec/client/render/api.json5 `device`), and in a build made for
  // measuring says what runs slower than perf.json5's LIMIT.SLOW_MS.
  const gpu = new WebGpu(
    link,
    { power: device.POWER, required_feats: device.REQUIRED_FEATURES, optional_feats: device.OPTIONAL_FEATURES },
    { upload_chunk_sz: device.UPLOAD_CHUNK_BYTES, slow_ms: __PERF_DRIVE__ ? LIMIT.SLOW_MS : null },
    (state) => tell({ kind: "gpu", state }),
  );
  gpu.serve();
  serve(m.port, link, m.named, m.prefixes, LATER, {
    give: (h, canvas) => gpu.bind(h, canvas as OffscreenCanvas),
    take: (h) => gpu.unbind(h),
  });
  link.init();
  await declare(link);
}

/** Declares the tile source to the core, and tells the page its state. */
async function declare(link: Side): Promise<void> {
  watch_source(() => tell({ kind: "source", state: source_state(), decl: source_decl() }));
  await declare_source({ send: (h, m, args) => wait(link, h, m, args) });
}
