// @xpute/runtime/abi/timer.ts
//
// GENERATED from wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  open as open_uri,
  refusal_of,
  type Ref,
  type Sender,
  uri,
} from "../link.ts";

/** The singleton's URI. */
export const URI = uri("xpute:core", "timer");

/** `sleep` */
export const SLEEP: Method = 0;

export interface Timer {
  sleep(ms: number): Promise<void>;
}

/** The object over `imp`. */
function object(l: Pick<Link, "complete">, imp: Timer): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case SLEEP: {
            if (args === null) return Errno.EBADMSG;
            imp.sleep(args.at(0).get_number()).then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          default:
            return Errno.ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Mounts `imp` at the URI, one object for every open. */
export function mount(l: Pick<Link, "mount" | "complete">, imp: Timer): void {
  const o = object(l, imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function sleep(ref: Ref, ms: number): Promise<void> {
  const a = await ref.link.send(ref.handle, SLEEP, [ms]);
  if (a.result < 0) throw new XputeError(a.result);
}
