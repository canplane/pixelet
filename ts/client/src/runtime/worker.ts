// @pxl/client/runtime/worker.ts

/**
 * A page's core and the host's link to it run in a worker of their own, so
 * that the core holds up none of the UI's events. The page sends its kept
 * entries, then the core's module, the UI's port and the named handles of the
 * interfaces the UI serves; the worker instantiates the module with a link laid over it
 * (@xpute/web-runtime `instantiate`), mounts the objects the host serves, the
 * core's files and GPU among them, and the UI's stand-ins, enters the core,
 * and serves the link to the UI across the port (@xpute/link remote.ts), the
 * canvases the UI gives going to the GPU. The files are here, so the source
 * is declared and the refused files are set aside here too.
 */

import { type Link, type Sender, serve, wait } from "@xpute/link";
import { serve_clock, serve_stop } from "@xpute/web-host";
import { forward_console, type Instantiate, instantiate } from "@xpute/web-runtime";
import { serve_vsync, WebGpu } from "@xpute/gpu-web-host";
import { device } from "@/abi/gpu.spec.ts";
import { mem } from "@/abi/manifest.spec.ts";
import { LIMIT } from "@/abi/perf.spec.ts";
import * as source from "@/abi/source.ts";
import type { ToPage, ToWorker } from "@/runtime/boot.ts";
import { set_online } from "@/service/origin/online.ts";
import { changed, serve_files } from "@/service/sys/fs/file.ts";
import { follow_refused_files, path_of } from "@/service/sys/fs/refusal.ts";
import { declare_source, source_decl, source_state, watch_source } from "@/service/sys/fs/source.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { serve_text } from "@/service/core/text.ts";
import * as online from "@/service/platform/online.ts";

const tell = (m: ToPage) => postMessage(m);

// What the worker writes to its console goes to the page's lines too, where a
// tool driving the page reads them (service/platform/log.ts).
forward_console(tell);

onmessage = ({ data: m }: MessageEvent<ToWorker>) => {
  switch (m.kind) {
    case "entries":
      return keep(m.entries);
    case "instantiate":
      return void start(m);
    case "refused":
      return follow_refused_files(m.files, (f) => changed(path_of(f)));
  }
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
  const link = await instantiate(m.module, m.init, mem.bytes, tell);
  if (link === null) return;
  serve_stop(link, (stop) => tell({ kind: "stopped", stop }));
  serve_clock(link);
  serve_vsync(link);
  serve_text(link);
  serve_files(link);
  // The page's host stages its writes in the chunks a device's does
  // (spec/client/render/api.json `device`), and in a build made for
  // measuring says what runs slower than perf.json's LIMIT.SLOW_MS.
  const gpu = new WebGpu(
    link,
    { power: device.POWER, required_feats: device.REQUIRED_FEATURES, optional_feats: device.OPTIONAL_FEATURES },
    { upload_chunk_bytes: device.UPLOAD_CHUNK_BYTES, slow_ms: __PERF_DRIVE__ ? LIMIT.SLOW_MS : null },
    (state) => tell({ kind: "gpu", state }),
  );
  gpu.serve();
  serve(m.port, link, m.named, {
    give: (h, canvas) => gpu.bind(h, canvas as OffscreenCanvas),
    take: (h) => gpu.unbind(h),
  });
  link.init();
  await declare(link);
}

/** Declares the tile source to the core, and tells the page its state. */
async function declare(link: Link): Promise<void> {
  const sender: Sender = { send: (h, m, args) => wait(link, h, m, args) };
  watch_source(() => tell({ kind: "source", state: source_state(), decl: source_decl() }));
  await declare_source((min_zoom, formats, shards) =>
    source.declare(sender, min_zoom, Uint32Array.from(formats), shards).then(
      () => true,
      () => false,
    ),
  );
}
