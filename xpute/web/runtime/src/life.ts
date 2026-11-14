// @xpute/web-runtime/life.ts

/** The page's lifecycle, told to a guest that serves it (wit/life/life.wit):
 * hidden on `visibilitychange` to hidden, the Page Lifecycle's last sure event,
 * and shown on visible. The worker tells it the UI attached (`tell_attached`).
 * The guest's answer is its own (`take_answer`). */

import { XputeError } from "@xpute/kit/status/error.ts";
import * as life from "@xpute/link/abi/life/life.ts";
import { type Side, type Sender, sender } from "@xpute/link";

/** Takes the guest's answer as its own, an error it answers too, ENOENT where
 * it serves none and EPIPE where it trapped, as a device's runtime takes it
 * (xpute-native-runtime life.rs). */
function take_answer(told: Promise<void>): void {
  told.catch((e: unknown) => {
    if (!(e instanceof XputeError)) throw e;
  });
}

export function tell_life(link: Sender): void {
  document.addEventListener("visibilitychange", () => {
    take_answer(
      document.visibilityState === "hidden"
        ? life.hidden(link)
        : life.shown(link),
    );
  });
}

/** Tells the guest on `link` the UI attached, once the UI's port is served
 * and the guest has booted, as a device's runtime tells it at each start
 * (xpute-native-runtime remote.rs `accept`). */
export function tell_attached(link: Side): void {
  take_answer(life.attached(sender(link)));
}
