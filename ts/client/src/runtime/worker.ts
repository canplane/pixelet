// @pxl/client/runtime/worker.ts

/**
 * A page's core and the runtime's link to it run in a worker of their own, so
 * that the core holds up none of the UI's events. The page sends the core's
 * module, the UI's port, the prefixes of the objects the UI serves and its
 * kept keys; the worker opens the module on the link, mounts the objects the
 * runtime serves, the core's files and GPU among them, and the UI's
 * stand-ins, enters the core, and serves the link to the UI across the port
 * (@xpute/runtime remote.ts), the canvases the UI gives going to the GPU. The files are here, so the source is declared and the refused
 * files are set aside here too.
 */

import { Link, type Sender, serve, serve_clock, serve_stop, serve_timer, wait } from "@xpute/runtime";
import { serve_vsync } from "@xpute/gpu/vsync.ts";
import { WebGpu } from "@xpute/gpu/web.ts";
import { device } from "@/abi/gpu.spec.ts";
import { MANIFEST } from "@/abi/manifest.spec.ts";
import { LIMIT } from "@/abi/perf.spec.ts";
import * as source from "@/abi/source.ts";
import type { Open, ToPage, ToWorker } from "@/runtime/boot.ts";
import { set_online } from "@/service/origin/online.ts";
import { changed, serve_files } from "@/service/sys/fs/file.ts";
import { follow_refused_files, path_of } from "@/service/sys/fs/refusal.ts";
import { declare_source, source_decl, source_state, watch_source } from "@/service/sys/fs/source.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { serve_text } from "@/service/core/text.ts";
import * as online from "@/service/platform/online.ts";

interface Exports {
  memory: WebAssembly.Memory;
  link_init(): number;
  link_boot(): void;
  link_in(): number;
}

const tell = (m: ToPage) => postMessage(m);

/** A console argument as text: an error with its stack. */
function line(a: unknown): string {
  if (a instanceof Error) return a.stack ?? a.message;
  try {
    return String(a);
  } catch {
    return Object.prototype.toString.call(a);
  }
}

// What the worker writes to its console goes to the page's lines too, where a
// tool driving the page reads them (service/platform/log.ts).
for (const level of ["log", "info", "warn", "error"] as const) {
  const write = console[level].bind(console);
  console[level] = (...args: unknown[]) => {
    write(...args);
    tell({ kind: "log", text: args.map(line).join(" ") });
  };
}

onmessage = ({ data: m }: MessageEvent<ToWorker>) => {
  switch (m.kind) {
    case "open":
      return void open(m);
    case "refused":
      return follow_refused_files(m.files, (f) => changed(path_of(f)));
  }
};

/** A copy of the page's kept keys, each write sent back to the page. */
function keep(keys: [string, string][]): void {
  const store = new Map(keys);
  set_storage({
    get: (key) => store.get(key) ?? null,
    set: (key, val) => {
      store.set(key, val);
      tell({ kind: "keep", key, val });
    },
  });
}

async function open(m: Open): Promise<void> {
  keep(m.keys);
  online.watch(set_online);
  let link!: Link;
  let x: Exports;
  try {
    const { instance } = await WebAssembly.instantiateStreaming(fetch(m.module, m.init), {
      env: Link.imports(() => link),
    });
    x = instance.exports as unknown as Exports;
    // The core's memory, whole before its first entry; it never grows after.
    x.memory.grow(Math.ceil((MANIFEST.mem_bytes - x.memory.buffer.byteLength) / 65536));
  } catch (e) {
    // The core never starts: its module did not load, or the browser refused
    // its memory.
    tell({ kind: "failed", err: line(e) });
    return;
  }
  // A trap out of any export stops the core for good: the page hears it, with
  // the error, and the worker closes, so nothing enters the core again.
  const entry = <T>(f: () => T) => (): T => {
    try {
      return f();
    } catch (e) {
      tell({ kind: "trap", err: line(e) });
      close();
      throw e;
    }
  };
  link = new Link({
    get mem() {
      return x.memory.buffer;
    },
    init: entry(x.link_init),
    boot: entry(x.link_boot),
    in: entry(x.link_in),
  });
  serve_stop(link, (stop) => tell({ kind: "stop", stop }));
  serve_timer(link);
  serve_clock(link);
  serve_vsync(link);
  serve_text(link);
  serve_files(link);
  // The page's runtime stages its writes in the chunks a device's does
  // (spec/client/render/api.json `device`), and in a build made for
  // measuring says what runs slower than perf.json's LIMIT.SLOW_MS.
  const gpu = new WebGpu(
    link,
    MANIFEST.gpu,
    { upload_chunk_bytes: device.UPLOAD_CHUNK_BYTES, slow_ms: __PERF_DRIVE__ ? LIMIT.SLOW_MS : null },
    (state) => tell({ kind: "gpu", state }),
  );
  gpu.serve();
  serve(m.port, link, m.prefixes, {
    give: (h, canvas) => gpu.bind(h, canvas as OffscreenCanvas),
    take: (h) => gpu.unbind(h),
  });
  link.init();
  await declare(link);
}

/** Declares the tile source to the core, and tells the page its state. */
async function declare(link: Link): Promise<void> {
  const sender: Sender = { send: (h, m, args) => wait(link, h, m, args) };
  const src = await source.open(sender);
  watch_source(() => tell({ kind: "source", state: source_state(), decl: source_decl() }));
  await declare_source((min_zoom, formats, shards) =>
    source.declare(src, min_zoom, formats, shards).then(
      () => true,
      () => false,
    ),
  );
}
