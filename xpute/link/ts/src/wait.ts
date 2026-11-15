// @xpute/link/wait.ts

/** A sent method as a promise, over `End.send`, resolved from its answer
 * (xpute-link wait.rs), and an end as a `Sender` through it. */

import type { Answered, Args, End, Handle, Method, Sender } from "./link.ts";

export function wait(
  link: End,
  h: Handle,
  m: Method,
  args: Args = null,
): Promise<Answered> {
  return new Promise((resolve) =>
    link.send(h, m, args, (result, data) => resolve({ result, data })),
  );
}

/** `end` as a `Sender`, each method sent through `wait`. */
export function sender(end: End): Sender {
  return { send: (h, m, args) => wait(end, h, m, args) };
}
