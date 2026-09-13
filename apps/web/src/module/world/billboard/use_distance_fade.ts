// @pxl/web/module/world/billboard/use_distance_fade.ts

import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { distance_fade_factor } from "./distance_fade.ts";
import { useTick } from "@/module/world/sched/tick.ts";

// Scratch, reused across every consumer's own useFrame call each frame —
// same reasoning camera/input.tsx's own $ray/$pointer module-level scratch
// objects exist for (avoid a per-frame, per-billboard allocation).
const $world_pos = new THREE.Vector3();

/**
 * Drives one SpriteBillboard's own scale/opacity/position from its live
 * distance to the camera, every frame, via the sprite's own ref — mutated in
 * place (not React state) the same way companion.tsx/location_puck.tsx do
 * their own per-frame updates. `width`/`height` are the sprite's own
 * full-strength size.
 *
 * `base_z`/`center_frac`: `base_z` is the local Z where this billboard's own
 * anchor point sits (a chat bubble's tail tip, a POI badge's dot center).
 * `center_frac` is where that anchor point falls within the sprite itself,
 * 0 (top) to 1 (bottom) — a THREE.Sprite scales around its own center, so
 * pinning any other point requires recomputing position.z every frame from
 * the live scale, not baking in a static offset once at mount (that would
 * drift as scale changes).
 */
export function use_distance_fade(
  sprite_ref: React.RefObject<THREE.Sprite | null>,
  width: f64,
  height: f64,
  base_z: f64,
  center_frac: f64,
): void {
  useTick("cosmetic", ({ state }) => {
    const sprite = sprite_ref.current;
    if (!sprite) return;
    sprite.getWorldPosition($world_pos);
    const dist = $world_pos.distanceTo(state.camera.position);

    // Pixels per unit at the sprite's view depth, not its distance: the
    // projection divides by depth, and fitting by distance drew a sprite off
    // the view axis larger by 1/cos of its angle.
    const cam = state.camera as THREE.PerspectiveCamera;
    const depth = Math.max(-$world_pos.applyMatrix4(cam.matrixWorldInverse).z, 1e-3);
    const px_per_world_unit = state.size.height / (2 * depth * Math.tan((cam.fov * Math.PI) / 360));

    const { scale, opacity } = distance_fade_factor(px_per_world_unit, dist);
    sprite.scale.set(width * scale, height * scale, 1);
    // Both halves of one fact: how far the sprite's centre sits above its
    // anchor. The position keeps the anchor pinned at base_z while the
    // sprite rescales; the uniform lets anchor_occlusion.ts's shader recover
    // the anchor from the centre it is given.
    const anchor_drop = height * scale * (center_frac - 0.5);
    sprite.position.z = base_z + anchor_drop;
    const drop = (sprite.material as THREE.Material).userData?.anchor_drop as { value: number } | undefined;
    if (drop) drop.value = anchor_drop;
    (sprite.material as THREE.SpriteMaterial).opacity = opacity;
  }, "distance_fade");
}
