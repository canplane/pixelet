// @pxl/client/abi/watch.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

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
import * as types from "./types.ts";

/** The singleton's URI. */
export const URI = uri("pxl:core", "watch");

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;

export interface Watch {
  start(sensor: types.Sensor): void;
  stop(sensor: types.Sensor): void;
}

/** The object over `imp`. */
function object(imp: Watch): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case START: {
            if (args === null) return Errno.EBADMSG;
            imp.start(args.at(0).get_number() as types.Sensor);
            return 0;
          }
          case STOP: {
            if (args === null) return Errno.EBADMSG;
            imp.stop(args.at(0).get_number() as types.Sensor);
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
export function mount(l: Pick<Link, "mount">, imp: Watch): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function start(ref: Ref, sensor: types.Sensor): Promise<void> {
  const a = await ref.link.send(ref.handle, START, [sensor]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function stop(ref: Ref, sensor: types.Sensor): Promise<void> {
  const a = await ref.link.send(ref.handle, STOP, [sensor]);
  if (a.result < 0) throw new XputeError(a.result);
}
