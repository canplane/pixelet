// @pxl/client/runtime/worker.ts

/**
 * A page's core and runtime run in a worker of their own, so that a turn
 * holds up none of the UI's events. The core is served on xpute's port once
 * the page has sent what a worker cannot read. The refused files are followed
 * here, where the files are.
 */

import { gpu_grant } from "@xpute/gpu/gpu.ts";
import { gpu_serve } from "@xpute/gpu/web.ts";
import { Major } from "@/abi/cmd.spec.ts";
import { type Link, serve } from "@xpute/runtime";
import { device } from "@/abi/gpu.spec.ts";
import { MANIFEST } from "@/abi/manifest.spec.ts";
import { LIMIT } from "@/abi/perf.spec.ts";
import {
  declare_source,
  source_decl,
  source_state,
  watch_source,
} from "@/service/sys/fs/source.ts";
import { follow_refused_files } from "@/service/sys/fs/refusal.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { set_online } from "@/service/origin/online.ts";
import { config } from "@/runtime/start.ts";
import { set_core } from "@/service/core/core.ts";
import type { Page, ToCore, ToUi } from "@/runtime/page.ts";
import { fs_changed } from "@/service/core/fs.ts";
import { source_declare } from "@/service/core/source.ts";
import { gpu } from "@/service/platform/gpu.ts";
import * as online from "@/service/platform/online.ts";

const tell = (m: ToUi) => postMessage(m);
const page = Promise.withResolvers<Page>();

onmessage = ({ data: m }: MessageEvent<ToCore>) => {
  switch (m.kind) {
    case "page":
      return page.resolve(m);
    case "gpu":
      return gpu_serve(m.port, gpu);
    case "refused":
      return follow_refused_files(m.files, fs_changed);
  }
};

async function keep(): Promise<boolean> {
  const { keys, handheld } = await page.promise;
  const store = new Map(keys);
  set_storage({
    get: (key) => store.get(key) ?? null,
    set: (key, val) => {
      store.set(key, val);
      tell({ kind: "keep", key, val });
    },
  });
  return handheld;
}

/** The core's commands are bound here too, for what sys/ sends from this
 * side. */
function up(link: Link): void {
  set_core(link);
  online.watch(set_online);
  watch_source(() =>
    tell({
      kind: "source",
      state: source_state(),
      decl: source_decl(),
    }),
  );
  void declare_source(source_declare);
}

void serve(MANIFEST, async () => config(await keep())).then((r) => {
  // The page's runtime stages its writes in the chunks a device's does
  // (spec/client/render/api.json `device`), and in a build made for
  // measuring says what runs slower than perf.json's LIMIT.SLOW_MS, as the
  // core has a device's runtime do (pxl-core lib.rs `GPU`).
  gpu_grant(gpu, r, MANIFEST.gpu, Major.GPU, {
    upload_chunk_bytes: device.UPLOAD_CHUNK_BYTES,
    slow_ms: __PERF_DRIVE__ ? LIMIT.SLOW_MS : null,
  });
  up(r);
});
