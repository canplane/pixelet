// @pxl/client/abi/watch.ts
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
import * as types from "./types.ts";

/** The interface's handle, named by `pxl:core/watch` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x71919e;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "watch");

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;

export interface Watch {
  start(sensor: types.Sensor): void;
  stop(sensor: types.Sensor): void;
}

/** The object over `imp`. */
function object(imp: Watch): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case START: {
            if (args === null) return Errno.EBADMSG;
            imp.start(args.at(0).get_number() as types.Sensor);
            return 0;
          }
          case STOP: {
            if (args === null) return Errno.EBADMSG;
            imp.stop(args.at(0).get_number() as types.Sensor);
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
export function mount(l: Pick<Link, "mount_named">, imp: Watch): void {
  l.mount_named(HANDLE, object(imp));
}

export async function start(link: Sender, sensor: types.Sensor): Promise<void> {
  const a = await link.send(HANDLE, START, [sensor]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function stop(link: Sender, sensor: types.Sensor): Promise<void> {
  const a = await link.send(HANDLE, STOP, [sensor]);
  if (a.result < 0) throw new XputeError(a.result);
}
