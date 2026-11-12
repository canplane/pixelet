// @pxl/client/runtime/worker.ts

/**
 * A page's core and the runtime's link to it run in a worker of their own, so
 * that the core holds up none of the UI's events. The page sends the core's
 * module, the UI's port and the prefixes of the objects the UI serves; the
 * worker opens the module on the link, mounts the objects the runtime serves
 * and the UI's stand-ins, enters the core, and serves the link to the UI
 * across the port (@xpute/runtime remote.ts).
 */

import { Link, serve, serve_clock, serve_timer } from "@xpute/runtime";
import { serve_vsync } from "@xpute/gpu/vsync.ts";
import type { Open } from "@/runtime/boot.ts";
import { serve_text } from "@/service/core/text.ts";

interface Exports {
  memory: WebAssembly.Memory;
  link_init(): number;
  link_in(): number;
}

onmessage = async ({ data: m }: MessageEvent<Open>) => {
  let link!: Link;
  const { instance } = await WebAssembly.instantiateStreaming(fetch(m.module, m.init), {
    env: Link.imports(() => link),
  });
  const x = instance.exports as unknown as Exports;
  link = new Link({
    get mem() {
      return x.memory.buffer;
    },
    init: x.link_init,
    in: x.link_in,
  });
  serve_timer(link);
  serve_clock(link);
  serve_vsync(link);
  serve_text(link);
  serve(m.port, link, m.prefixes);
  link.init();
};
