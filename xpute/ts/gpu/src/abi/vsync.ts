// @xpute/gpu/abi/vsync.ts
//
// GENERATED from wit/gpu (xpute-gen spec/wit.rs)

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
} from "@xpute/runtime";

/** The singleton's URI. */
export const URI = uri("xpute:gpu", "vsync");

/** `next` */
export const NEXT: Method = 0;

export interface Vsync {
  next(): Promise<void>;
}

/** The object over `imp`. */
function object(l: Pick<Link, "complete">, imp: Vsync): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NEXT: {
            imp.next().then(
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
export function mount(l: Pick<Link, "mount" | "complete">, imp: Vsync): void {
  const o = object(l, imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function next(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, NEXT, []);
  if (a.result < 0) throw new XputeError(a.result);
}
