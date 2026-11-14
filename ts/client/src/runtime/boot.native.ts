// @pxl/client/runtime/boot.native.ts

/**
 * A device's core and xpute's runtime are linked into the app, and the runtime
 * runs the core on a thread of its own, serving it the stop, the clocks, the
 * text, the GPU and the settings on the disk; the app's lifecycle reaches it
 * natively. The UI opens its end of the runtime's link here
 * (@xpute/native-runtime `open_remote`), reaches the core through the device's
 * core (service/platform/core.native.ts), and serves the core, on its own
 * thread, what a page's worker serves (worker.ts): the files, the objects, and
 * the source's declaration, whose state is the UI's here. The GPU's host tells
 * the UI what it has of a GPU through its own module, as a page's worker does.
 * Where the boot stands is moved as on a page (ui.ts).
 */

import { state_on } from "@xpute/gpu-native-runtime";
import { open_remote, stopped_on, trap_on } from "@xpute/native-runtime";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { serve_files } from "@/service/sys/fs/file.ts";
import { serve_objects } from "@/service/sys/fs/object.ts";
import { declare_source } from "@/service/sys/fs/source.ts";
import { core_of } from "@/service/platform/core.ts";
import { serve_ui, start_client, trapped } from "@/runtime/ui.ts";

export function boot(): void {
  const { remote, start, link } = core_of(open_remote());
  stopped_on(stopped);
  trap_on(() => trapped("the guest trapped"));
  state_on((state) => set_gpu(GPU_STATE[state]));
  serve_ui(remote);
  serve_files(link);
  serve_objects(link);
  start();
  start_client();
  void declare_source(remote);
}
