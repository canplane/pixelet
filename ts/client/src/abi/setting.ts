// @pxl/client/abi/setting.ts
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

/** The singleton's URI. */
export const URI = uri("pxl:core", "setting");

/** `dev` */
export const DEV: Method = 0;
/** `set` */
export const SET: Method = 1;

export interface Setting {
  dev(dev: boolean): void;
  set(key: number, val: number): void;
}

/** The object over `imp`. */
function object(imp: Setting): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DEV: {
            if (args === null) return Errno.EBADMSG;
            imp.dev(args.at(0).get_bool());
            return 0;
          }
          case SET: {
            if (args === null) return Errno.EBADMSG;
            imp.set(args.at(0).get_number(), args.at(1).get_number());
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
export function mount(link: Pick<Link, "mount">, imp: Setting): void {
  const o = object(imp);
  link.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function dev(ref: Ref, dev: boolean): Promise<void> {
  const a = await ref.link.send(ref.handle, DEV, [dev]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function set(ref: Ref, key: number, val: number): Promise<void> {
  const a = await ref.link.send(ref.handle, SET, [key, val]);
  if (a.result < 0) throw new XputeError(a.result);
}
