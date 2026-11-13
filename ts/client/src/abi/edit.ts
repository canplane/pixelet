// @pxl/client/abi/edit.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/edit` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x453522;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "edit");

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

/** The name of the `Tool` member numbered `n`, or undefined for a number
 * no member has. */
export function tool_name(n: number): keyof typeof Tool | undefined {
  return (Object.keys(Tool) as (keyof typeof Tool)[]).find(
    (k) => Tool[k] === n,
  );
}

/** The `Tool` member `n` is, or null for a number no member has. */
export function tool_of(n: number): Tool | null {
  return tool_name(n) === undefined ? null : (n as Tool);
}

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

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Edit): void {
  l.mount_named(HANDLE, object(imp));
}

export async function hold(
  link: Sender,
  tool: Tool,
  val: number,
  scale_steps: number,
): Promise<void> {
  const a = await link.send(HANDLE, HOLD, [tool, val, scale_steps]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_text(link: Sender, text: string): Promise<void> {
  const a = await link.send(HANDLE, MARKER_TEXT, [text]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_cancel(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, MARKER_CANCEL, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function marker_commit(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, MARKER_COMMIT, []);
  if (a.result < 0) throw new XputeError(a.result);
}
