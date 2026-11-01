// @pxl/client/port/scene.ts

/** The scene's commands: a character drawn alone into a surface the shell
 * lent, and a line said. */

import type { u32, u8 } from "@xpute/kit/abi/word.ts";
import {
  Command,
  encode_scene_chat,
  encode_scene_preview_close,
  encode_scene_preview_open,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

export function kernel_preview_open(
  surface: u32,
  group: u32,
  number: u8,
): void {
  HOST.now(
    port.SHELL,
    Command.SCENE_PREVIEW_OPEN,
    encode_scene_preview_open(HOST.doorbell, surface, group, number),
  );
}

export function kernel_preview_close(surface: u32): void {
  HOST.now(
    port.SHELL,
    Command.SCENE_PREVIEW_CLOSE,
    encode_scene_preview_close(HOST.doorbell, surface),
  );
}

export function kernel_chat(text: string): void {
  HOST.now(
    port.SHELL,
    Command.SCENE_CHAT,
    encode_scene_chat(HOST.doorbell, text),
  );
}

/** Whether a line fits a command; one too long to send is refused whole. */
export function kernel_chat_fits(text: string): boolean {
  return HOST.doorbell.fits(encode_scene_chat(HOST.doorbell, text));
}
