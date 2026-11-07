// @pxl/client/runtime/worker.ts

/**
 * A page's kernel and runtime, in a worker of their own so a turn holds none
 * of the shell's events (open.ts starts it). The kernel is served
 * on xpute's port once the page has told what a worker cannot read; the source
 * is declared here and mirrored to the shell; the refused files are followed
 * here, where the files are; the profiler is read here.
 */

import { type Link, serve } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  declare_source,
  source_declaration,
  source_state,
  watch_source,
} from "@/sys/fs/source.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { set_storage } from "@/sys/fs/storage.ts";
import { set_online } from "@/origin/online.ts";
import { config, perf_here } from "@/runtime/start.ts";
import { set_kernel } from "@/port/kernel.ts";
import type { Page, ToKernel, ToShell } from "@/runtime/page.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import * as online from "@/service/online.ts";

const tell = (m: ToShell) => postMessage(m);
const page = Promise.withResolvers<Page>();

onmessage = ({ data: m }: MessageEvent<ToKernel>) => {
  switch (m.kind) {
    case "page":
      return page.resolve(m);
    case "refused":
      return follow_refused_files(m.files);
    case "perf":
      return tell({
        kind: "perf",
        id: m.id,
        value: answer(m.what as PerfRead, m.args),
      });
  }
};

/** Whether the page is held in the hand, its kept keys taken here first,
 * each one written told the page. */
async function keep(): Promise<boolean> {
  const { keys, handheld } = await page.promise;
  const kept = new Map(keys);
  set_storage({
    get: (key) => kept.get(key) ?? null,
    set: (key, value) => {
      kept.set(key, value);
      tell({ kind: "keep", key, value });
    },
  });
  return handheld;
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
  core: async () => config(await keep()),
}).then(up);
