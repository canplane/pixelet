// @xpute/runtime/abi/stop.ts
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
export const URI = uri("xpute:core", "stop");

/** `stopped` */
export const STOPPED: Method = 0;

export interface Stop {
  stopped(
    errnum: number,
    file: string,
    line: number,
    n1: bigint,
    n2: bigint,
  ): void;
}

/** The object over `imp`. */
function object(imp: Stop): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case STOPPED: {
            if (args === null) return Errno.EBADMSG;
            imp.stopped(
              args.at(0).get_number(),
              args.at(1).get_str(),
              args.at(2).get_number(),
              args.at(3).get_u64(),
              args.at(4).get_u64(),
            );
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
export function mount(l: Pick<Link, "mount">, imp: Stop): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function stopped(
  ref: Ref,
  errnum: number,
  file: string,
  line: number,
  n1: bigint,
  n2: bigint,
): Promise<void> {
  const a = await ref.link.send(ref.handle, STOPPED, [
    errnum,
    file,
    line,
    n1,
    n2,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}
