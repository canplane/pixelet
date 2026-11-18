// @xpute/runtime-web/life.ts

/** The page's lifecycle, told to a guest that implements it
 * (wit/life/life.wit) from its start (page.ts `start`), as the native module
 * tells its runtime the app's: hidden on `visibilitychange` to hidden, the
 * Page Lifecycle's last sure event, and shown on visible. The worker tells it
 * the UI attached (`tell_attached`). The guest's answer is its own
 * (`take_answer`). */

import { XputeError } from "@xpute/kit/status/error.ts";
import * as life from "@xpute/host/abi/life/life.ts";
import { type Sender, type Side } from "@xpute/link/link.ts";
import { sender } from "@xpute/link/wait.ts";

/** Takes the guest's answer as its own, an error it answers too, ENOENT where
 * it implements none and EPIPE where it trapped, as the native runtime takes
 * it (xpute-runtime-native life.rs). */
function take_answer(told: Promise<void>): void {
  told.catch((e: unknown) => {
    if (!(e instanceof XputeError)) throw e;
  });
}

/** Tells the guest the page's lifecycle over `link`, the UI's end. */
export function tell_life(link: Sender): void {
  document.addEventListener("visibilitychange", () => {
    take_answer(
      document.visibilityState === "hidden"
        ? life.hidden(link)
        : life.shown(link),
    );
  });
}

/** Tells the guest on `side` the UI attached, once the UI's port is attached
 * and the guest has booted, as the native runtime tells it as each UI
 * attaches (xpute-runtime-native remote.rs `accept`). */
export function tell_attached(side: Side): void {
  take_answer(life.attached(sender(side)));
}
