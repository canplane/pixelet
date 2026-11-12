// @xpute/runtime/abi/clock.ts
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
export const URI = uri("xpute:core", "clock");

/** `now` */
export const NOW: Method = 0;

export interface Clock {
  now(at: number): void;
}

/** The object over `imp`. */
function object(imp: Clock): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NOW: {
            if (args === null) return Errno.EBADMSG;
            imp.now(args.at(0).get_number());
            return 0;
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
export function mount(l: Pick<Link, "mount">, imp: Clock): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function now(ref: Ref, at: number): Promise<void> {
  const a = await ref.link.send(ref.handle, NOW, [at]);
  if (a.result < 0) throw new XputeError(a.result);
}
