// @pxl/client/runtime/worker.ts

/**
 * A page's core and runtime run in a worker of their own, so that a turn
 * holds up none of the UI's events. The core is served on xpute's port once
 * the page has sent what a worker cannot read. The refused files are followed
 * here, where the files are, and the profiler is read here, beside the core's
 * memory.
 */

import { type Link, serve } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  declare_source,
  source_decl,
  source_state,
  watch_source,
} from "@/service/sys/fs/source.ts";
import { follow_refused_files } from "@/service/sys/fs/refusal.ts";
import { set_storage } from "@/service/sys/fs/storage.ts";
import { set_online } from "@/service/origin/online.ts";
import { config, perf_here } from "@/runtime/start.ts";
import { set_core } from "@/service/core/core.ts";
import type { Page, ToCore, ToUi } from "@/runtime/page.ts";
import { fs_changed } from "@/service/core/fs.ts";
import { answer, type PerfRead } from "@/service/core/perf.ts";
import { source_declare } from "@/service/core/source.ts";
import * as online from "@/service/platform/online.ts";

const tell = (m: ToUi) => postMessage(m);
const page = Promise.withResolvers<Page>();

onmessage = ({ data: m }: MessageEvent<ToCore>) => {
  switch (m.kind) {
    case "page":
      return page.resolve(m);
    case "refused":
      return follow_refused_files(m.files, fs_changed);
    case "perf":
      return tell({
        kind: "perf",
        id: m.id,
        val: answer(m.what as PerfRead, m.args),
      });
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
  set_core(link, perf_here);
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

void serve({
  machine: MACHINE,
  core: async () => config(await keep()),
}).then(up);
