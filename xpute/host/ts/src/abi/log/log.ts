// @xpute/host/abi/log/log.ts
//
// GENERATED from wit/log (xpute-link-gen wit.rs)

import { type Level, of_Level } from "./log.enum.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:log/log` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x73da26;

/** `log` */
export const LOG: Method = 0;

export function load_Level(r: Lift, at: number): Level {
  const v = of_Level(r.case(at, 6));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface Host {
  log(level: Level, context: string, msg: string): void;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, _tag, args, _ret) {
      try {
        switch (m) {
          case LOG: {
            implementation.log(load_Level(args, 0), args.str(4), args.str(12));
            return 0;
          }
          default:
            return ERRNO_ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
    drop() {},
  };
}

/** Mounts `implementation` under the interface's handle. */
export function mount(
  l: Pick<Callee, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
