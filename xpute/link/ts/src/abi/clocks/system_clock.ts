// @xpute/link/abi/clocks/system_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  Branch,
  TreeReader,
  type NodeCursor,
} from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "../../link.ts";

/** The interface's handle, named by `xpute:clocks/system-clock` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7dfa61;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:clocks", "system-clock");

/** `now` */
export const NOW: Method = 0;

export interface Instant {
  seconds: bigint;
  nanoseconds: number;
}

export function write_Instant(r: Instant): Branch {
  return new Branch([BigInt.asUintN(64, r.seconds), r.nanoseconds]);
}

export function read_Instant(c: NodeCursor): Instant {
  const b = c.as_branch();
  return {
    seconds: BigInt.asIntN(64, b.at(0).get_u64()),
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
            return { result: 0, data: [write_Instant(v)] };
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

export async function now(link: Sender): Promise<Instant> {
  const a = await link.send(HANDLE, NOW, []);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return read_Instant(new TreeReader(a.data).read_branch().at(0));
}
