// @pxl/client/runtime/boot.ts

/**
 * Boots the client from the bottom up, on its platform's services (service/).
 * It opens the runtime where the platform places it: on a page in the core's
 * worker (open.ts), and on a device beside the UI (open.native.ts). It binds
 * the core's commands to the runtime. Then it moves where the boot stands as
 * the runtime starts, the GPU answers, the world's source is declared and the
 * core's snapshots arrive. The UI takes no part in the boot. It reads where
 * the boot stands through the port (port/ui.ts), and, once the core has
 * started, what the core shows it.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { GroundStatus, type Snapshot } from "@/abi/cmd.spec.ts";
import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
} from "@/abi/state.ts";
import { boot_state, set_boot } from "@/port/boot.ts";
import * as snapshot from "@/port/snapshot.ts";
import { open } from "@/runtime/open.ts";

let called = false;

/** Whether the world has arrived, or nothing of it will show: every pipeline
 * is made, and the view is drawn or lies past the service area. When locating
 * fails, the view is drawn at its default place. */
function loaded(s: Snapshot): boolean {
  return (
    s.renderer_progress === 1 &&
    (s.pivot.status === GroundStatus.Success || s.pivot.outside)
  );
}

/** Called once by the app, ahead of the UI (src/boot.ts). `boot_state` says
 * where the boot stands. A second call is refused. */
export function boot(): void {
  if (called) throw new InvariantError(Errno.EALREADY);
  called = true;
  open({
    started: () => {
      if (boot_state() === BOOT_OPENING) set_boot(BOOT_STARTING);
    },
    snapshot: () => {
      if (boot_state() === BOOT_LOADING && loaded(snapshot.current()))
        set_boot(BOOT_STARTED);
    },
    // When the core stops, its turns end with it, whatever the boot's state.
    terminated: () => set_boot(BOOT_TERMINATED),
  }).then(
    (declared) => {
      if (declared && boot_state() === BOOT_STARTING) set_boot(BOOT_LOADING);
    },
    (err) => {
      console.error("[boot] the client did not start:", err);
      const state = boot_state();
      if (state === BOOT_OPENING || state === BOOT_STARTING)
        set_boot(BOOT_FAILED);
    },
  );
}
