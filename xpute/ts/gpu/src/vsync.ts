// @xpute/gpu/vsync.ts

/** The display's vsync the runtime serves the core beside its GPU
 * (wit/gpu/vsync.wit), on the platform's animation frames. */

import type { Link } from "@xpute/runtime/link.ts";
import * as vsync from "./abi/vsync.ts";

export function serve_vsync(link: Link): void {
  vsync.mount(link, { next: () => new Promise((done) => requestAnimationFrame(() => done())) });
}
