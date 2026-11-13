// @xpute/link/abi/life/life.ts
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "../../link.ts";

/** The interface's handle, named by `xpute:life/life` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x46edb9;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:life", "life");

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

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Life): void {
  l.mount_named(HANDLE, object(imp));
}

export async function hidden(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, HIDDEN, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function shown(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, SHOWN, []);
  if (a.result < 0) throw new XputeError(a.result);
}
