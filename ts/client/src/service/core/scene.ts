// @pxl/client/service/core/scene.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Command, SceneContent } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

export function draw_world(surface: u32): void {
  core.send(Command.SCENE_DRAW, surface, SceneContent.World, null, null);
}

/** Draws the companion's character of `group` alone into `surface`, as it
 * looks in the world, over the color `bg` (0xRRGGBB). A second draw into the
 * same surface replaces the first. */
export function draw_character(surface: u32, group: u32, bg: u32): void {
  core.send(Command.SCENE_DRAW, surface, SceneContent.Character, group, bg);
}

export function chat(text: string): void {
  core.send(Command.SCENE_CHAT, text);
}

/** A line too long for one command is refused whole, never cut. */
export function chat_fits(text: string): boolean {
  return core.fits(Command.SCENE_CHAT, text);
}
