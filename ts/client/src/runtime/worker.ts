// @pxl/client/runtime/worker.ts

/**
 * On the web the core and the host's link to it run in a worker of their
 * own, so that the core holds up none of the UI's events. The page sends, in
 * one message, its kept entries, the core's module, the UI's port, and the
 * named handles and prefixes of what the UI implements; the worker
 * instantiates the module with a link laid over it (@xpute/runtime-web
 * `instantiate`) and runs the core on it from the mounts listed here
 * (@xpute/runtime-web `run`): what the host implements, the core's files and
 * GPU among them, the UI's stand-ins across the port, the canvases the UI
 * gives going to the GPU. The files are here, so the source is declared here
 * too.
 */

import { type Side, sender } from "@xpute/link";
import { mount_clock, mount_stop } from "@xpute/host";
import { type Instantiate, instantiate, run } from "@xpute/runtime-web";
import { mount_vsync, WebGpu } from "@pxl/gpu/host";
import { device } from "@/abi/gpu.spec.ts";
import { IMPORTS } from "@/abi/imports.ts";
import { CALLS } from "@/abi/calls.ts";
import { set_online } from "@/service/origin/online.ts";
import { mount_files } from "@/service/sys/fs/file.ts";
import { mount_objects } from "@/service/sys/fs/object.ts";
import {
  type Declaration,
  declare_source,
  type Source,
  source_decl,
  source_state,
  watch_source,
} from "@/service/sys/fs/source.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { mount_text } from "@/service/core/text.ts";
import * as online from "@/service/platform/online.ts";

/** What the page's start sends the worker beside the core's instantiation
 * (@xpute/runtime-web `Instantiate`): the page's kept entries, which a worker
 * cannot read. */
export interface Kept {
  entries: [string, string][];
}

/** The source's state, as the worker tells the page. */
export interface SourceReport {
  kind: "source";
  state: Source;
  decl: Declaration | null;
}

/** A kept key the worker wrote, which the page keeps too. */
export interface KeepReport {
  kind: "keep";
  key: string;
  val: string;
}

const tell = (m: SourceReport | KeepReport) => postMessage(m);

onmessage = ({ data: m }: MessageEvent<Instantiate & Kept>) => {
  keep(m.entries);
  void boot(m);
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

async function boot(m: Instantiate): Promise<void> {
  online.watch(set_online);
  const instance = await instantiate(m.module, m.init);
  if (instance === null) return;
  const { side } = instance;
  // The web host runs by the config the native one does
  // (spec/client/render/api.json5 `device`).
  const gpu = new WebGpu(
    side,
    {
      power: device.POWER,
      required_feats: device.REQUIRED_FEATURES,
      optional_feats: device.OPTIONAL_FEATURES,
    },
    {
      frame_latency: device.FRAME_LATENCY,
      upload_chunk_sz: device.UPLOAD_CHUNK_SZ,
    },
  );
  run(
    instance,
    {
      mounts: [
        mount_stop,
        mount_clock,
        mount_vsync,
        () => gpu.mount(),
        (s) => mount_text(s, s),
        (s) => mount_files(s, s),
        (s) => mount_objects(s, s),
      ],
      imports: IMPORTS,
      calls: CALLS,
      taker: {
        give: (h, canvas) => gpu.bind(h, canvas as OffscreenCanvas),
        take: (h) => gpu.unbind(h),
      },
    },
    m,
  );
  await declare(side);
}

/** Declares the tile source to the core, and tells the page its state. */
async function declare(side: Side): Promise<void> {
  watch_source(() =>
    tell({ kind: "source", state: source_state(), decl: source_decl() }),
  );
  await declare_source(sender(side));
}
