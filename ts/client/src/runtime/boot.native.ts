// @pxl/client/runtime/boot.native.ts

/**
 * A device's core and xpute's runtime are linked into the app, and the runtime
 * runs the core on a thread of its own, serving it the stop, the clocks, the
 * text and the GPU; the app's lifecycle reaches it natively. The UI opens its
 * end of the runtime's link and starts the runtime here (@xpute/native-runtime
 * `open_remote`, `start`), and serves the core, on its own thread, what a
 * page's worker serves (worker.ts): the files, the settings among them in the
 * platform's storage (storage.native.ts), and the objects, over the core's
 * memory, where the guest lends their buffers, and the source's declaration,
 * whose state is the UI's here. The GPU's host tells the UI what it has of a
 * GPU through its own module, as a page's worker does.
 * Where the boot stands is moved as on a page (ui.ts).
 */

import { state_on } from "@xpute/gpu-native-runtime";
import type { Side } from "@xpute/link";
import {
  bytes,
  open_remote,
  start,
  stopped_on,
  trap_on,
} from "@xpute/native-runtime";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { serve_files } from "@/service/sys/fs/file.ts";
import { serve_objects } from "@/service/sys/fs/object.ts";
import { declare_source } from "@/service/sys/fs/source.ts";
import { serve_ui, start_client, trapped } from "@/runtime/ui.ts";

export function boot(): void {
  const remote = open_remote();
  const link: Pick<
    Side,
    "bytes" | "mount" | "mount_named" | "task_return" | "on_cancel"
  > = {
    bytes,
    mount: (prefix, o) => remote.mount(prefix, o),
    mount_named: (h, o) => remote.mount_named(h, o),
    task_return: (t, result, data) => remote.task_return(t, result, data),
    on_cancel: (t, stop) => remote.on_cancel(t, stop),
  };
  stopped_on(stopped);
  trap_on(() => trapped("the guest trapped"));
  state_on((state) => set_gpu(GPU_STATE[state]));
  serve_ui(remote);
  serve_files(link);
  serve_objects(link);
  start(remote);
  start_client();
  void declare_source(remote);
}
