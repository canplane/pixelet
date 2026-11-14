// @xpute/link/wait.ts

/** A sent method as a promise, over `Side.send`, resolved from its answer
 * (xpute-link wait.rs), and a side as a `Sender` through it. */

import type { Answered, Args, Handle, Side, Method, Sender } from "./link.ts";

export function wait(
  link: Side,
  h: Handle,
  m: Method,
  args: Args = null,
): Promise<Answered> {
  return new Promise((resolve) =>
    link.send(h, m, args, (result, data) => resolve({ result, data })),
  );
}

/** `link` as a `Sender`, each method sent through `wait`. */
export function sender(link: Side): Sender {
  return { send: (h, m, args) => wait(link, h, m, args) };
}
