// @xpute/gpu-web-host/vsync.ts

/** The display's vsync the host serves the guest beside its GPU
 * (gpu/wit/vsync.wit), on the platform's animation frames. */

import type { Side } from "@xpute/link/link.ts";
import * as vsync from "@xpute/gpu-link/abi/vsync.ts";

export function serve_vsync(link: Side): void {
  vsync.mount(link, {
    next: () => new Promise((done) => requestAnimationFrame(() => done())),
  });
}
