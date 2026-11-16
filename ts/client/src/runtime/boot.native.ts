// @pxl/client/runtime/boot.native.ts

/**
 * A device's core and xpute's runtime are linked into the app, and the
 * runtime runs the core on a thread of its own, implementing its stop, its
 * clocks, its text and its GPU; the app's lifecycle reaches it natively. The
 * UI opens its end of the runtime and starts the runtime here (@xpute/expo
 * `open_ui`, `start`), and implements for the core, on its own thread, what a
 * page's worker implements (worker.ts): the files, the settings among them in
 * the platform's storage (storage.native.ts), and the objects, over the
 * core's memory, where the guest lends their buffers, and the source's
 * declaration, whose state is the UI's here. The GPU's host tells the UI what
 * it has of a GPU through its own module, as a page's worker does. Where the
 * boot stands is moved as on a page (ui.ts).
 */

import { state_on } from "@pxl/gpu/expo";
import { sender } from "@xpute/link";
import { open_ui, start, stopped_on, trap_on } from "@xpute/expo";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { mount_files } from "@/service/sys/fs/file.ts";
import { mount_objects } from "@/service/sys/fs/object.ts";
import { declare_source } from "@/service/sys/fs/source.ts";
import { mount_ui, start_client, trapped } from "@/runtime/ui.ts";

export function boot(): void {
  const ui = open_ui();
  stopped_on(stopped);
  trap_on(trapped);
  state_on((state) => set_gpu(GPU_STATE[state]));
  mount_ui(ui);
  mount_files(ui, ui);
  mount_objects(ui, ui);
  start();
  start_client();
  void declare_source(sender(ui));
}
