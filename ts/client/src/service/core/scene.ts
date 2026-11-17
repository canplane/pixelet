// @pxl/client/service/core/scene.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ComponentError } from "@xpute/link";
import * as scene from "@/abi/scene.ts";
import {
  SCENE_CONTENT_WORLD,
  SCENE_CONTENT_CHARACTER,
} from "@/abi/scene.enum.ts";
import { ui } from "@/service/core/core.ts";
import { worldOpen } from "@/service/core/sensor.ts";

export function drawWorld(surface: u32): void {
  void scene
    .draw(ui, surface, SCENE_CONTENT_WORLD, null, null)
    .catch(warnError);
  worldOpen(surface);
}

/** Draws the companion's character of `group` alone into `surface`, as it
 * looks in the world, over the color `bg` (0xRRGGBB). A second draw into the
 * same surface replaces the first. */
export function drawCharacter(surface: u32, group: u32, bg: u32): void {
  void scene
    .draw(ui, surface, SCENE_CONTENT_CHARACTER, group, bg)
    .catch(warnError);
}

/** Logs a draw's error case: its surface not open, or drawing another
 * scene. */
function warnError(e: unknown): void {
  if (!(e instanceof ComponentError)) throw e;
  console.warn(`[scene] not drawn: ${e.payload}`);
}

/** Resolves once the core takes the line, and rejects when it refuses it
 * whole: too long, a `ComponentError`. */
export function chat(text: string): Promise<void> {
  return scene.chat(ui, text);
}
