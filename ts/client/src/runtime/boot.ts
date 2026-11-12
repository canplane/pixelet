// @pxl/client/runtime/boot.ts

/**
 * A page opens the core in a worker of its own (worker.ts) and reaches it
 * across a message port. The UI takes no part in the boot: it reads where the
 * boot stands (service/sys/boot.ts), and, once the core has started, what the
 * core shows it.
 */

import { Remote } from "@xpute/runtime";
import { BOOT_STARTING } from "@/abi/state.ts";
import { set_boot } from "@/service/sys/boot.ts";
import { mount_ui, open_core } from "@/service/core/core.ts";
import { source } from "@/service/platform/core.ts";

/** What the page sends the worker to open the core: the module, how to
 * fetch it, the UI's port, and the prefixes of the objects the UI serves. */
export interface Open {
  module: string;
  init: RequestInit | undefined;
  port: MessagePort;
  prefixes: Remote["prefixes"];
}

export function boot(): void {
  // Metro bundles the worker through this call, and through no form of
  // `new Worker` that tree shaking also compiles (metro.config.ts).
  const worker = new Worker(require.unstable_resolveWorker("./worker.ts"), {
    type: "module",
  });
  const { port1, port2 } = new MessageChannel();
  const { module, init } = source();
  const remote = new Remote(port1);
  mount_ui(remote);
  worker.postMessage(
    { module: module.href, init, port: port2, prefixes: remote.prefixes } satisfies Open,
    [port2],
  );
  void open_core(remote);
  set_boot(BOOT_STARTING);
}
