// @xpute/runtime/abi/life.ts
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
export const URI = uri("xpute:core", "life");

/** `hidden` */
export const HIDDEN: Method = 0;
/** `shown` */
export const SHOWN: Method = 1;

export interface Life {
  hidden(): void;
  shown(): void;
}

/** The object over `imp`. */
function object(imp: Life): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case HIDDEN: {
            imp.hidden();
            return 0;
          }
          case SHOWN: {
            imp.shown();
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
export function mount(link: Pick<Link, "mount">, imp: Life): void {
  const o = object(imp);
  link.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function hidden(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, HIDDEN, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function shown(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, SHOWN, []);
  if (a.result < 0) throw new XputeError(a.result);
}
