// @xpute/runtime/timer.ts

/** The timer the runtime serves the core (wit/timer.wit), on the event loop's
 * own timers. */

import * as timer from "./abi/timer.ts";
import type { Link } from "./link.ts";

export function serve_timer(link: Link): void {
  timer.mount(link, { sleep: (ms) => new Promise((done) => setTimeout(done, ms)) });
}
