// @pxl/client/abi/scene.ts
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

/** The interface's handle, named by `pxl:core/scene` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7c289a;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "scene");

/** `draw` */
export const DRAW: Method = 0;
/** `chat` */
export const CHAT: Method = 1;

export const SceneContent = {
  World: 0,
  Character: 1,
} as const;
export type SceneContent = (typeof SceneContent)[keyof typeof SceneContent];

/** The name of the `SceneContent` member numbered `n`, or undefined for a number
 * no member has. */
export function scene_content_name(
  n: number,
): keyof typeof SceneContent | undefined {
  return (Object.keys(SceneContent) as (keyof typeof SceneContent)[]).find(
    (k) => SceneContent[k] === n,
  );
}

/** The `SceneContent` member `n` is, or null for a number no member has. */
export function scene_content_of(n: number): SceneContent | null {
  return scene_content_name(n) === undefined ? null : (n as SceneContent);
}

export interface Scene {
  draw(
    surface: number,
    content: SceneContent,
    group: number | null,
    bg: number | null,
  ): void;
  chat(text: string): void;
}

/** The object over `imp`. */
function object(imp: Scene): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DRAW: {
            if (args === null) return Errno.EBADMSG;
            imp.draw(
              args.at(0).get_number(),
              args.at(1).get_number() as SceneContent,
              args.at(2).get_opt((c) => c.get_number()),
              args.at(3).get_opt((c) => c.get_number()),
            );
            return 0;
          }
          case CHAT: {
            if (args === null) return Errno.EBADMSG;
            imp.chat(args.at(0).get_str());
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
export function mount(l: Pick<Link, "mount_named">, imp: Scene): void {
  l.mount_named(HANDLE, object(imp));
}

export async function draw(
  link: Sender,
  surface: number,
  content: SceneContent,
  group: number | null,
  bg: number | null,
): Promise<void> {
  const a = await link.send(HANDLE, DRAW, [surface, content, group, bg]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function chat(link: Sender, text: string): Promise<void> {
  const a = await link.send(HANDLE, CHAT, [text]);
  if (a.result < 0) throw new XputeError(a.result);
}
