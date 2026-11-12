// @xpute/runtime/clock.ts

/** The clock the runtime serves the core (wit/clock.wit). */

import * as clock from "./abi/clock.ts";
import type { Link } from "./link.ts";

export function serve_clock(link: Link): void {
  clock.mount(link, {
    now(at) {
      const b = link.bytes(at, 16);
      const v = new DataView(b.buffer, b.byteOffset, 16);
      v.setFloat64(0, performance.now(), true);
      v.setFloat64(8, Date.now(), true);
    },
  });
}
