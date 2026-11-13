// @xpute/link/abi/clocks/system_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  ScalarType,
  type NodeCursor,
  type PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
  get_as,
} from "../../link.ts";

/** The interface's handle, named by `xpute:clocks/system-clock` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7dfa61;

/** `now` */
export const NOW: Method = 0;

export interface Instant {
  seconds: bigint;
  nanoseconds: number;
}

export function write_Instant(w: PacketWriter, r: Instant): void {
  w.branch(2, (w) => {
    w.i64(r.seconds);
    w.u32(r.nanoseconds);
  });
}

export function read_Instant(c: NodeCursor): Instant {
  const b = c.as_branch();
  return {
    seconds: get_as<bigint>(b.at(0), ScalarType.I64),
    nanoseconds: b.at(1).get_number(),
  };
}

export interface SystemClock {
  now(): Instant;
}

/** The object over `imp`. */
function object(imp: SystemClock): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NOW: {
            const v = imp.now();
            return {
              result: 0,
              data: [
                1,
                (w) => {
                  write_Instant(w, v);
                },
              ],
            };
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
export function mount(l: Pick<Link, "mount_named">, imp: SystemClock): void {
  l.mount_named(HANDLE, object(imp));
}
