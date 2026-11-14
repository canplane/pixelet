// @xpute/link/abi/clocks/system_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { Lift, Lower } from "../../cabi.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "../../link.ts";
import { ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";

/** The interface's handle, named by `xpute:clocks/system-clock` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x7dfa61;

/** `now` */
export const NOW: Method = 0;

export interface Instant {
  seconds: bigint;
  nanoseconds: number;
}

export const INSTANT_SZ = 16;
export const INSTANT_ALIGN = 8;

export function store_Instant(w: Lower, at: number, v: Instant): void {
  w.i64(at, v.seconds);
  w.u32(at + 8, v.nanoseconds);
}

export function load_Instant(r: Lift, at: number): Instant {
  return {
    seconds: r.i64(at),
    nanoseconds: r.u32(at + 8),
  };
}

export interface Host {
  now(): Instant;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NOW: {
            const v = implementation.now();
            return {
              result: 0,
              data: [
                INSTANT_SZ,
                (w) => {
                  store_Instant(w, 0, v);
                },
              ],
            };
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

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
