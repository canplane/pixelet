// @pxl/client/abi/scene.ts
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
export const URI = uri("pxl:core", "scene");

/** `draw` */
export const DRAW: Method = 0;
/** `chat` */
export const CHAT: Method = 1;

export const SceneContent = {
  World: 0,
  Character: 1,
} as const;
export type SceneContent = (typeof SceneContent)[keyof typeof SceneContent];

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

/** Mounts `imp` at the URI, one object for every open. */
export function mount(link: Pick<Link, "mount">, imp: Scene): void {
  const o = object(imp);
  link.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function draw(
  ref: Ref,
  surface: number,
  content: SceneContent,
  group: number | null,
  bg: number | null,
): Promise<void> {
  const a = await ref.link.send(ref.handle, DRAW, [
    surface,
    content,
    group,
    bg,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function chat(ref: Ref, text: string): Promise<void> {
  const a = await ref.link.send(ref.handle, CHAT, [text]);
  if (a.result < 0) throw new XputeError(a.result);
}
