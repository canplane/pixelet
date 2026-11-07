// @pxl/client/port/scene.ts

/** The scene's commands: what a surface the shell lent draws, the world or a
 * character alone, and a line said. */

import type { u32, u8 } from "@xpute/kit/abi/word.ts";
import { encode_scene_chat, SceneContent } from "@/abi/cmd.spec.ts";
import { kernel, ui } from "@/runtime/kernel.ts";

/** The world drawn into `surface`. */
export function draw_world(surface: u32): void {
  kernel.scene_draw(surface, SceneContent.World, null, null, null);
}

/** A character drawn alone into `surface`, over `bg`, 0xRRGGBB; a second one
 * replaces it. */
export function draw_character(
  surface: u32,
  group: u32,
  number: u8,
  bg: u32,
): void {
  kernel.scene_draw(surface, SceneContent.Character, group, number, bg);
}

export function chat(text: string): void {
  kernel.scene_chat(text);
}

/** Whether a line fits a command; one too long to send is refused whole. */
export function chat_fits(text: string): boolean {
  return ui.fits((room) => encode_scene_chat(room, text));
}
