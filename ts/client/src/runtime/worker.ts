// @pxl/client/runtime/worker.ts

/**
 * A page's kernel and runtime, in a worker of their own so a turn holds none
 * of the shell's events (service/runtime.ts starts it). The kernel is served
 * on xpute's port once the page's kept keys have come; the source is declared
 * here and mirrored to the shell; the profiler is read here.
 */

import { type Link, serve } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  declare_source,
  source_declaration,
  source_state,
  watch_source,
} from "@/sys/fs/source.ts";
import { set_storage } from "@/sys/fs/storage.ts";
import { set_online } from "@/origin/online.ts";
import { config, perf_here } from "@/runtime/start.ts";
import { set_kernel } from "@/runtime/kernel.ts";
import type { ToKernel, ToShell } from "@/runtime/page.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import * as online from "@/service/online.ts";

const tell = (m: ToShell) => postMessage(m);
const keys = Promise.withResolvers<[string, string][]>();

onmessage = ({ data: m }: MessageEvent<ToKernel>) => {
  switch (m.kind) {
    case "keys":
      return keys.resolve(m.keys);
    case "perf":
      return tell({
        kind: "perf",
        id: m.id,
        value: answer(m.what as PerfRead, m.args),
      });
  }
};

/** The page's kept keys, each one written told the page. */
async function keep(): Promise<void> {
  const kept = new Map(await keys.promise);
  set_storage({
    get: (key) => kept.get(key) ?? null,
    set: (key, value) => {
      kept.set(key, value);
      tell({ kind: "keep", key, value });
    },
  });
}

/** Up: the kernel's commands bound here too, for what sys/ sends; the source
 * declared here, and mirrored to the shell. */
function up(link: Link): void {
  set_kernel(link, perf_here);
  online.watch(set_online);
  watch_source(() =>
    tell({
      kind: "source",
      state: source_state(),
      declaration: source_declaration(),
    }),
  );
  void declare_source();
}

void serve({
  machine: MACHINE,
  core: async () => (await keep(), config()),
}).then(up);
