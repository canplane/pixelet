// @pxl/client/service/core/scene.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ComponentError } from "@xpute/link";
import * as scene from "@/abi/scene.ts";
import {
  SCENE_CONTENT_WORLD,
  SCENE_CONTENT_CHARACTER,
} from "@/abi/scene.enum.ts";
import { ui } from "@/service/core/core.ts";

export function draw_world(surface: u32): void {
  void scene.draw(ui, surface, SCENE_CONTENT_WORLD, null, null).catch(warn_err);
}

/** Draws the companion's character of `group` alone into `surface`, as it
 * looks in the world, over the color `bg` (0xRRGGBB). A second draw into the
 * same surface replaces the first. */
export function draw_character(surface: u32, group: u32, bg: u32): void {
  void scene
    .draw(ui, surface, SCENE_CONTENT_CHARACTER, group, bg)
    .catch(warn_err);
}

/** Logs a draw's error case: its surface not open, or drawing another
 * scene. */
function warn_err(e: unknown): void {
  if (!(e instanceof ComponentError)) throw e;
  console.warn(`[scene] not drawn: ${e.payload}`);
}

/** Resolves once the core takes the line, and rejects when it refuses it
 * whole, too long for the link. */
export function chat(text: string): Promise<void> {
  return scene.chat(ui, text);
}

/** Whether a chat line of `text` fits one call to the core: a line too long
 * for the link is refused whole, never cut (@xpute/link `Remote.fits`). */
export function chat_fits(text: string): boolean {
  return ui.fits((link) => scene.chat(link, text));
}
