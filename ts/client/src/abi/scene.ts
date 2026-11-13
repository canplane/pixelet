// @pxl/client/abi/scene.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/scene` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7c289a;

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
  for (const k in SceneContent)
    if (SceneContent[k as keyof typeof SceneContent] === n)
      return k as keyof typeof SceneContent;
  return undefined;
}

/** The `SceneContent` member `n` is, or null for a number no member has. */
export function scene_content_of(n: number): SceneContent | null {
  return scene_content_name(n) === undefined ? null : (n as SceneContent);
}

export async function draw(
  link_: Sender,
  surface: number,
  content: SceneContent,
  group: number | null,
  bg: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, DRAW, [
    4,
    (w_) => {
      w_.u32(surface);
      w_.u32(content);
      if (group === null) w_.nil();
      else {
        w_.u8(group);
      }
      if (bg === null) w_.nil();
      else {
        w_.u32(bg);
      }
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function chat(link_: Sender, text: string): Promise<void> {
  const a_ = await link_.send(HANDLE, CHAT, [
    1,
    (w_) => {
      w_.str(text);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}
