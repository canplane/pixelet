// @pxl/client/abi/perf.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
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
export const URI = uri("pxl:core", "perf");

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;
/** `stats` */
export const STATS: Method = 2;
/** `rings` */
export const RINGS: Method = 3;
/** `strtab` */
export const STRTAB: Method = 4;

export interface Perf {
  start(): void;
  stop(): void;
  stats(window_ms: number | null): U8Array;
  rings(ring: number, from: number, cnt: number): U8Array;
  strtab(): U8Array;
}

/** The object over `imp`. */
function object(imp: Perf): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case START: {
            imp.start();
            return 0;
          }
          case STOP: {
            imp.stop();
            return 0;
          }
          case STATS: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.stats(args.at(0).get_opt((c) => c.get_number()));
            return { result: 0, data: [v] };
          }
          case RINGS: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.rings(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
            );
            return { result: 0, data: [v] };
          }
          case STRTAB: {
            const v = imp.strtab();
            return { result: 0, data: [v] };
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
export function mount(l: Pick<Link, "mount">, imp: Perf): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function start(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, START, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function stop(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, STOP, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function stats(
  ref: Ref,
  window_ms: number | null,
): Promise<U8Array> {
  const a = await ref.link.send(ref.handle, STATS, [window_ms]);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return new TreeReader(a.data).read_branch().at(0).get() as U8Array;
}

export async function rings(
  ref: Ref,
  ring: number,
  from: number,
  cnt: number,
): Promise<U8Array> {
  const a = await ref.link.send(ref.handle, RINGS, [ring, from, cnt]);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return new TreeReader(a.data).read_branch().at(0).get() as U8Array;
}

export async function strtab(ref: Ref): Promise<U8Array> {
  const a = await ref.link.send(ref.handle, STRTAB, []);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return new TreeReader(a.data).read_branch().at(0).get() as U8Array;
}
