// @xpute/link/wait.ts

/** A sent method as a promise, over `Link.send`, resolved from its answer
 * (xpute-link wait.rs). */

import type { Answered, Args, Handle, Link, Method } from "./link.ts";

export function wait(link: Link, h: Handle, m: Method, args: Args = null): Promise<Answered> {
  return new Promise((resolve) => link.send(h, m, args, (result, data) => resolve({ result, data })));
}
