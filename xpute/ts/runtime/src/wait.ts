// @xpute/runtime/wait.ts

/** A sent method as a promise, over `Link.send`, resolved from its answer
 * (xpute-core wait.rs). */

import type { Arg } from "@xpute/kit/wire/xtp/mod.ts";
import type { Answered, Handle, Link, Method } from "./link.ts";

export function wait(link: Link, h: Handle, m: Method, args: readonly Arg[] = []): Promise<Answered> {
  return new Promise((resolve) => link.send(h, m, args, (result, data) => resolve({ result, data })));
}
