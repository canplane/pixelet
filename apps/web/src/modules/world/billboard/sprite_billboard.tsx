// @pxl/web/modules/world/billboard/sprite_billboard.tsx

/**
 * The reusable half of tier 3 (world-anchored billboards — see
 * discussions/20260907-world-rendering-tiers.md): a canvas-drawn texture
 * rendered as a camera-facing THREE.Sprite, `depthTest={false}` + a high
 * `renderOrder` so it reads as UI floating over the world rather than real
 * occludable geometry. Extracted from chat_bubble.tsx (2026-09-07); POI
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

import { forwardRef } from "react";
import type * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";

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
  /** 999 matches chat_bubble.tsx's own original value — high enough to
   * reliably win against real world geometry once depthTest is off, without
   * colliding with any other reserved renderOrder in this codebase (the
   * only other one found, markers/frame_gizmo.tsx's DEBUG_RENDER_ORDER, is
   * dev-only and uses the same 9999 hover_cursor.tsx's own terrain-case
   * ghost does — this tier's own convention is a full order of magnitude
   * below that, not competing with it). */
  renderOrder?: number;
};

export const SpriteBillboard = forwardRef<THREE.Sprite, SpriteBillboardProps>(
  function SpriteBillboard({ texture, width, height, position, renderOrder = 999 }, ref) {
    return (
      <sprite ref={ref} position={position ?? [0, 0, 0]} scale={[width, height, 1]} renderOrder={renderOrder}>
        {
          // depthTest off + high renderOrder: this tier reads as UI, not as a
          // real object the terrain/buildings should be able to hide — see
          // this file's own top doc comment.
        }
        <spriteMaterial map={texture} transparent depthTest={false} />
      </sprite>
    );
  },
);
