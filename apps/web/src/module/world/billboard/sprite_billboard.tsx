// @pxl/web/module/world/billboard/sprite_billboard.tsx

/**
 * The reusable half of tier 3 (world-anchored billboards — see
 * issue/20260907-world-rendering-tiers.md): a canvas-drawn texture
 * rendered as a camera-facing THREE.Sprite, occluded by the world it stands
 * in (anchor_occlusion.ts) with a high `renderOrder` deciding the rest.
 * Extracted from chat_bubble.tsx (2026-09-07); POI
 * badges (poi/poi_layer.tsx) are this tier's second real consumer, same day.
 * What stays OUT of this file on purpose: any content-specific
 * canvas drawing (chat_bubble.tsx's notched speech-bubble border/tail is
 * chat-shaped, not billboard-shaped in general — a POI marker will likely
 * draw a pin/icon instead) and any positioning opinion (this component
 * takes a plain local `position`; whether that position comes from riding a
 * parent Actor's own transform, the way chat_bubble.tsx uses it, or from a
 * standalone `AnchorNode` wrapper, the way poi_layer.tsx uses it, is the
 * caller's own choice — this file has no opinion on anchoring).
 *
 * forwardRef (2026-09-07): exposes the underlying THREE.Sprite so a caller
 * needing per-frame imperative control can reach `sprite.scale` and
 * `sprite.material.opacity` directly — see billboard/use_distance_fade.ts,
 * the one real consumer, which both poi_layer.tsx and chat_bubble.tsx use
 * for their own distance-based scale/opacity fade, driven from useFrame
 * rather than React state (the same "mutate a ref, skip the re-render"
 * convention companion.tsx/location_puck.tsx already use for their own
 * every-frame updates).
 */

import { forwardRef, useEffect, useMemo } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { apply_anchor_occlusion } from "./anchor_occlusion.ts";

export type SpriteBillboardProps = {
  /** Built by use_canvas_texture (this directory) or an equivalent — this
   * component doesn't own the texture's lifecycle (creation/disposal), only
   * its render. */
  texture: THREE.Texture;
  /** World units, not canvas pixels — the same "logical size * world units
   * per pixel" conversion chat_bubble.tsx's own WORLD_UNITS_PER_PX already
   * did before this extraction; callers keep owning that conversion. */
  width: f64;
  height: f64;
  /** Local offset within whatever transform this sits under (a parent
   * Actor's own group, or an AnchorNode's) — defaults to that transform's
   * own origin. */
  position?: readonly [f64, f64, f64];
  /** 10000 — originally 999 (chat_bubble.tsx's own value before this
   * extraction), raised once tier 4's own hover_cursor.tsx
   * terrain-case ghost (renderOrder 9999) and the dev-only
   * marker/frame_gizmo.tsx debug overlay (also 9999) were both confirmed
   * live to draw over a billboard sharing its screen position — a lower
   * renderOrder loses that draw-order race regardless of depthTest, and
   * tier 3 (informational UI: chat/POI) reading as *more* topmost than
   * tier 4's one-off spatial indicators is the correct hierarchy anyway.
   * Kept one order of magnitude above every other reserved renderOrder in
   * this codebase found at the time, not just barely above it, so a future
   * indicator doesn't reopen the same race by picking something in
   * between. */
  renderOrder?: number;
};

export const SpriteBillboard = forwardRef<THREE.Sprite, SpriteBillboardProps>(
  function SpriteBillboard({ texture, width, height, position, renderOrder = 10000 }, ref) {
    // One material per billboard rather than one shared: use_distance_fade
    // writes `material.opacity` per sprite, so they cannot share an instance.
    // Built here rather than declared as <spriteMaterial> because the anchor
    // patch is an onBeforeCompile hook, which R3F has no prop for.
    const material = useMemo(() => apply_anchor_occlusion(new THREE.SpriteMaterial({ transparent: true, depthTest: true, depthWrite: false })), []);
    useEffect(() => () => material.dispose(), [material]);
    material.map = texture;

    return (
      <sprite ref={ref} position={position ?? [0, 0, 0]} scale={[width, height, 1]} renderOrder={renderOrder}>
        {
          // Two occlusion questions, both in anchor_occlusion.ts. Whether the
          // labelled point is visible at all is decided once at the anchor,
          // all-or-nothing. Whether something stands between the camera and
          // the badge itself is left to the GPU's depth test, which works
          // here only because the shader writes the depth of an upright twin
          // of this quad: a Sprite's own depth is the anchor's for every
          // fragment, and testing that punches a band of terrain through the
          // middle of a badge instead of hiding it behind a hill.
          //
          // depthWrite off too — without it this sprite wrote real depth
          // values, so a later-drawn, depth-testing indicator
          // (hover_cursor.tsx's ghost) could pass its own depth test against
          // them and paint over a billboard whose renderOrder said it should
          // already be on top.
        }
        <primitive object={material} attach="material" />
      </sprite>
    );
  },
);
