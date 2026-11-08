// @pxl/client/port/scene.ts

/** The scene's commands: what a surface the UI lent draws, either the world
 * or one group's companion character alone, and a line of chat said. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { encode_scene_chat, SceneContent } from "@/abi/cmd.spec.ts";
import { core, ui } from "@/port/core.ts";

export function draw_world(surface: u32): void {
  core.scene_draw(surface, SceneContent.World, null, null);
}

/** Draws the companion's character of `group` alone into `surface`, as it
 * looks in the world, over the color `bg` (0xRRGGBB). A second draw into the
 * same surface replaces the first. */
export function draw_character(surface: u32, group: u32, bg: u32): void {
  core.scene_draw(surface, SceneContent.Character, group, bg);
}

export function chat(text: string): void {
  core.scene_chat(text);
}

/** Whether a line fits in one command. A line too long to send is refused
 * whole, never cut. */
export function chat_fits(text: string): boolean {
  return ui.fits((room) => encode_scene_chat(room, text));
}
