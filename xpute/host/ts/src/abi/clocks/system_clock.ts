// @xpute/host/abi/clocks/system_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { i64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import type { Lift, Lower } from "@xpute/link/cabi.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:clocks/system-clock` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x7dfa61;

/** `now` */
export const NOW: Method = 0;

export interface Instant {
  seconds: i64;
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
    call(m, _tag, _args, ret) {
      try {
        switch (m) {
          case NOW: {
            const v = implementation.now();
            ret.start(INSTANT_SZ);
            store_Instant(ret, 0, v);
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
