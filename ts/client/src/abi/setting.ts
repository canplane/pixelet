// @pxl/client/abi/setting.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

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
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/setting` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x6f63cb;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "setting");

/** `dev` */
export const DEV: Method = 0;
/** `set` */
export const SET: Method = 1;

export interface Setting {
  dev(dev: boolean): void;
  set(key: number, val: number): void;
}

/** The object over `imp`. */
function object(imp: Setting): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DEV: {
            if (args === null) return Errno.EBADMSG;
            imp.dev(args.at(0).get_bool());
            return 0;
          }
          case SET: {
            if (args === null) return Errno.EBADMSG;
            imp.set(args.at(0).get_number(), args.at(1).get_number());
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
export function mount(l: Pick<Link, "mount_named">, imp: Setting): void {
  l.mount_named(HANDLE, object(imp));
}

export async function dev(link: Sender, dev: boolean): Promise<void> {
  const a = await link.send(HANDLE, DEV, [dev]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function set(
  link: Sender,
  key: number,
  val: number,
): Promise<void> {
  const a = await link.send(HANDLE, SET, [key, val]);
  if (a.result < 0) throw new XputeError(a.result);
}
