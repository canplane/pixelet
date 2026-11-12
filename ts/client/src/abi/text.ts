// @pxl/client/abi/text.ts
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
export const URI = uri("pxl:core", "text");

/** `layout` */
export const LAYOUT: Method = 0;
/** `raster` */
export const RASTER: Method = 1;

export interface Text {
  layout(
    weight: number,
    px: number,
    max_width: number,
    at: number,
    lines: number,
    text_at: number,
    text_len: number,
  ): number;
  raster(
    weight: number,
    px: number,
    color: number,
    at: number,
    width: number,
    height: number,
    x: number,
    text_at: number,
    text_len: number,
  ): number;
}

/** The object over `imp`. */
function object(imp: Text): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case LAYOUT: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.layout(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
              args.at(3).get_number(),
              args.at(4).get_number(),
              args.at(5).get_number(),
              args.at(6).get_number(),
            );
            return v;
          }
          case RASTER: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.raster(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
              args.at(3).get_number(),
              args.at(4).get_number(),
              args.at(5).get_number(),
              args.at(6).get_number(),
              args.at(7).get_number(),
              args.at(8).get_number(),
            );
            return v;
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
export function mount(link: Pick<Link, "mount">, imp: Text): void {
  const o = object(imp);
  link.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function layout(
  ref: Ref,
  weight: number,
  px: number,
  max_width: number,
  at: number,
  lines: number,
  text_at: number,
  text_len: number,
): Promise<number> {
  const a = await ref.link.send(ref.handle, LAYOUT, [
    weight,
    px,
    max_width,
    at,
    lines,
    text_at,
    text_len,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function raster(
  ref: Ref,
  weight: number,
  px: number,
  color: number,
  at: number,
  width: number,
  height: number,
  x: number,
  text_at: number,
  text_len: number,
): Promise<number> {
  const a = await ref.link.send(ref.handle, RASTER, [
    weight,
    px,
    color,
    at,
    width,
    height,
    x,
    text_at,
    text_len,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}
