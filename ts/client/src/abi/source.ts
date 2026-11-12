// @pxl/client/abi/source.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Branch } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  open as open_uri,
  refusal_of,
  type Ref,
  type Sender,
  uri,
} from "@xpute/runtime";

/** The singleton's URI. */
export const URI = uri("pxl:core", "source");

/** `declare` */
export const DECLARE: Method = 0;

export interface Source {
  declare(min_zoom: number, format: number[], shard: string[]): void;
}

/** The object over `imp`. */
function object(imp: Source): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DECLARE: {
            if (args === null) return Errno.EBADMSG;
            imp.declare(
              args.at(0).get_number(),
              args
                .at(1)
                .as_branch()
                .children()
                .map((c) => c.get_number()),
              args
                .at(2)
                .as_branch()
                .children()
                .map((c) => c.get_str()),
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
export function mount(l: Pick<Link, "mount">, imp: Source): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function declare(
  ref: Ref,
  min_zoom: number,
  format: number[],
  shard: string[],
): Promise<void> {
  const a = await ref.link.send(ref.handle, DECLARE, [
    min_zoom,
    new Branch(format.map((x) => x)),
    new Branch(shard.map((x) => x)),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}
