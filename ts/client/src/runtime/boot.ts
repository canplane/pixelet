// @pxl/client/runtime/boot.ts

/**
 * Boots the client, bottom up, on its platform's services (service/): the
 * runtime opened where its platform places it (open.ts, a page's in the
 * core's worker; open.native.ts, a device's beside the UI), the core's
 * commands bound to it, then where the boot stands as the runtime starts, the
 * GPU answers, the world's source is declared and the core's snapshots come.
 * The UI is no part of it: it reads where the boot stands through the port
 * (port/ui.ts), and what the core shows it once it has started.
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

/** Whether the world has come, or nothing will show of it: every pipeline made,
 * and the view drawn or past the service area. A failed location draws the view
 * at its default. */
function loaded(s: Snapshot): boolean {
  return (
    s.renderer_progress === 1 &&
    (s.pivot.status === GroundStatus.Success || s.pivot.outside)
  );
}

/** Called once by the app, ahead of the UI (src/boot.ts); where it stands
 * is `boot_state`. A second call is refused. */
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
    // Its turns end with it.
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
