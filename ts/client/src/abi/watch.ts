// @pxl/client/abi/watch.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Sensor } from "./types.enum.ts";
import { load_Sensor } from "./types.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/watch` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x71919e;

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;

export interface Host {
  start(sensor: Sensor): void;
  stop(sensor: Sensor): void;
}

/** The object over `imp`. */
function object(imp: Host): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case START: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            imp.start(load_Sensor(r_, 0));
            return 0;
          }
          case STOP: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            imp.stop(load_Sensor(r_, 0));
            return 0;
          }
          default:
            return ERRNO_ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Host): void {
  l.mount_named(HANDLE, object(imp));
}
