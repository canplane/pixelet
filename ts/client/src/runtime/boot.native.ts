// @pxl/client/runtime/boot.native.ts

/**
 * Natively, the core and xpute's runtime are linked into the app, and the
 * runtime runs the core on a thread of its own, implementing its stop, its
 * clocks, its text and its GPU; the app's lifecycle reaches it natively. The
 * UI opens its end of the runtime and starts the runtime here (@xpute/expo
 * `open_ui`, `start`), and implements for the core, on its own thread, what the
 * web worker implements (worker.ts): the files, the settings among them in
 * the platform's storage (storage.native.ts), and the object source, over the
 * core's memory, where the guest lends their buffers, and the source's
 * declaration, whose state is the UI's here. The GPU's host tells the UI what
 * it has of a GPU through its own module, as the web worker does. Where the
 * boot stands is moved as on the web (ui.ts).
 */

import { state_on } from "@pxl/gpu/expo";
import { sender } from "@xpute/link";
import { open_ui, start, stopped_on, trap_on } from "@xpute/expo";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, setGpu } from "@/service/sys/gpu.ts";
import { mountFiles } from "@/service/sys/fs/file.ts";
import { mountObjects } from "@/service/sys/fs/object.ts";
import { declareSource } from "@/service/sys/fs/source.ts";
import { setStorage } from "@/service/sys/fs/storage.ts";
import * as storage from "@/service/platform/storage.ts";
import { mountUi, startClient, trapped } from "@/runtime/ui.ts";

export function boot(): void {
  const ui = open_ui();
  stopped_on(stopped);
  trap_on(trapped);
  state_on((state) => setGpu(GPU_STATE[state]));
  mountUi(ui);
  setStorage(storage);
  mountFiles(ui, ui);
  mountObjects(ui, ui);
  start();
  startClient();
  void declareSource(sender(ui));
}
