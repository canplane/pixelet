// @pxl/web/modules/world/billboard/billboard.tsx

/**
 * The single render path for tier 3 (world-anchored billboards — see
 * discussions/20260907-world-rendering-tiers.md). Owns everything a
 * billboard consumer (chat_bubble.tsx, poi_layer.tsx) shouldn't have to
 * repeat: canvas -> texture, world-unit scale, distance fade, and the
 * Sprite render. A consumer supplies only `built` (its own canvas, drawn
 * however it wants — design) and `base_z` (where its own anchor point sits
 * — placement); everything else is shared, not duplicated per file.
 */

import { useRef } from "react";
import type * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { WORLD_UNITS_PER_PX } from "./canvas_scale.ts";
import { use_canvas_texture } from "./use_canvas_texture.ts";
import { use_distance_fade } from "./use_distance_fade.ts";
import { SpriteBillboard } from "./sprite_billboard.tsx";

export type BuiltBillboard = {
  canvas: HTMLCanvasElement;
  logical_w: f64;
  logical_h: f64;
  /** Canvas-space Y (0 = top, `logical_h` = bottom) of this billboard's own
   * anchor point — the point that stays pinned at `base_z` regardless of
   * scale. A chat bubble's tail tip is its own bottom (`logical_h`); a POI
   * badge's anchor dot is its own center, partway up from the bottom. */
  center_y: f64;
};

export type BillboardProps = {
  built: BuiltBillboard | null;
  /** Where `built.center_y` sits, in the parent transform's local Z — see
   * use_distance_fade's own `base_z` doc comment. */
  base_z: f64;
};

export function Billboard({ built, base_z }: BillboardProps) {
  const texture = use_canvas_texture(built?.canvas ?? null);
  const sprite_ref = useRef<THREE.Sprite>(null);
  const width = (built?.logical_w ?? 0) * WORLD_UNITS_PER_PX;
  const height = (built?.logical_h ?? 0) * WORLD_UNITS_PER_PX;
  const center_frac = built && built.logical_h > 0 ? built.center_y / built.logical_h : 1;
  use_distance_fade(sprite_ref, width, height, base_z, center_frac);

  if (!built || !texture) return null;
  return <SpriteBillboard ref={sprite_ref} texture={texture} width={width} height={height} position={[0, 0, base_z]} />;
}
