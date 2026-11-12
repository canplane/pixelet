// @pxl/client/abi/edit.ts
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
export const URI = uri("pxl:core", "edit");

/** `hold` */
export const HOLD: Method = 0;
/** `marker-text` */
export const MARKER_TEXT: Method = 1;
/** `marker-cancel` */
export const MARKER_CANCEL: Method = 2;
/** `marker-commit` */
export const MARKER_COMMIT: Method = 3;

export const Tool = {
  Pointer: 0,
  Item: 1,
  Eraser: 2,
} as const;
export type Tool = (typeof Tool)[keyof typeof Tool];

export interface Edit {
  hold(tool: Tool, val: number, scale_steps: number): void;
  marker_text(text: string): void;
  marker_cancel(): void;
  marker_commit(): void;
}

/** The object over `imp`. */
function object(imp: Edit): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case HOLD: {
            if (args === null) return Errno.EBADMSG;
            imp.hold(
              args.at(0).get_number() as Tool,
              args.at(1).get_number(),
              args.at(2).get_number(),
            );
            return 0;
          }
          case MARKER_TEXT: {
            if (args === null) return Errno.EBADMSG;
            imp.marker_text(args.at(0).get_str());
            return 0;
          }
          case MARKER_CANCEL: {
            imp.marker_cancel();
            return 0;
          }
          case MARKER_COMMIT: {
            imp.marker_commit();
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
export function mount(l: Pick<Link, "mount">, imp: Edit): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function hold(
  ref: Ref,
  tool: Tool,
  val: number,
  scale_steps: number,
): Promise<void> {
  const a = await ref.link.send(ref.handle, HOLD, [tool, val, scale_steps]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_text(ref: Ref, text: string): Promise<void> {
  const a = await ref.link.send(ref.handle, MARKER_TEXT, [text]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_cancel(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, MARKER_CANCEL, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_commit(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, MARKER_COMMIT, []);
  if (a.result < 0) throw new XputeError(a.result);
}
