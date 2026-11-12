// @xpute/gpu/abi/gpu.ts
//
// GENERATED from wit/gpu (xpute-gen spec/wit.rs)

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
} from "@xpute/runtime";
import * as link from "@xpute/runtime/abi/link.ts";

/** The singleton's URI. */
export const URI = uri("xpute:gpu", "gpu");

/** `run` */
export const RUN: Method = 0;
/** `[method]device.lost` */
export const DEVICE_LOST: Method = 0;
/** `[method]device.ready` */
export const DEVICE_READY: Method = 1;
/** `[method]device.settled` */
export const DEVICE_SETTLED: Method = 2;

export interface Gpu {
  run(records: link.Lent): Promise<void>;
}

/** The object over `imp`. */
function object(l: Pick<Link, "complete">, imp: Gpu): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case RUN: {
            if (args === null) return Errno.EBADMSG;
            imp.run(link.read_Lent(args.at(0))).then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
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
export function mount(l: Pick<Link, "mount" | "complete">, imp: Gpu): void {
  const o = object(l, imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function run(ref: Ref, records: link.Lent): Promise<void> {
  const a = await ref.link.send(ref.handle, RUN, [link.write_Lent(records)]);
  if (a.result < 0) throw new XputeError(a.result);
}

export interface Device {
  lost(block: link.Lent): Promise<void>;
  ready(): Promise<number>;
  settled(): Promise<void>;
}

/** The object over `imp`. */
export function device(l: Pick<Link, "complete">, imp: Device): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DEVICE_LOST: {
            if (args === null) return Errno.EBADMSG;
            imp.lost(link.read_Lent(args.at(0))).then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case DEVICE_READY: {
            imp.ready().then(
              (v) => l.complete(tag, v, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case DEVICE_SETTLED: {
            imp.settled().then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
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

export async function device_lost(ref: Ref, block: link.Lent): Promise<void> {
  const a = await ref.link.send(ref.handle, DEVICE_LOST, [
    link.write_Lent(block),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function device_ready(ref: Ref): Promise<number> {
  const a = await ref.link.send(ref.handle, DEVICE_READY, []);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function device_settled(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, DEVICE_SETTLED, []);
  if (a.result < 0) throw new XputeError(a.result);
}
