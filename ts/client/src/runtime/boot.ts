// @pxl/client/runtime/boot.ts

/**
 * A page opens the runtime in the core's worker (open.ts), and a device
 * opens it beside the UI (open.native.ts). The UI takes no part in the boot.
 * It reads where the boot stands (service/sys/boot.ts), and, once the core
 * has started, what the core shows it.
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
import { boot_state, set_boot } from "@/service/sys/boot.ts";
import * as snapshot from "@/service/core/snapshot.ts";
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

export function boot(): void {
  if (called) throw new InvariantError(Errno.EALREADY);
  called = true;
  open({
    started: () => {
      if (boot_state() === BOOT_OPENING) set_boot(BOOT_STARTING);
    },
    snapshot: () => {
      if (boot_state() === BOOT_LOADING && loaded(snapshot.cur()))
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
