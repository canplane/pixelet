// @pxl/web/module/camera/orbit.ts

/**
 * Pure math for this app's orbit camera model, pulled out of use_camera.ts /
 * use_view_frame.ts so it can be unit tested on its own. Doc'd once here
 * because the model is easy to get backwards:
 *
 * **Azimuth never rotates the camera.** Only `dist` and `polar` determine
 * camera.position (orbit_local_offset below), and the camera always looks
 * straight at its target. World content rotates by azimuth under this fixed
 * camera instead (module/world/frame/view.ts, stepped by use_view_frame.ts) — the
 * ground spins, not the eye. Anything needing "which way is the camera
 * effectively facing" (the sky dome's sun/moon sweep in environment.tsx) has
 * no single camera transform to read that off of — only this rotate2
 * primitive, signed for the conversion direction: world_frame_ref's own
 * rotation.z is always exactly +azimuth, so matching the ground's own
 * transform means rotate2(..., +azimuth), and un-rotating a camera-local
 * point back to its ground origin means rotate2(..., -azimuth).
 */

import type { f64 } from "@xpute/core/kit/word.ts";

/**
 * The camera's local-space offset from its look-at target, given the current
 * (already-damped/clamped) dist/polar. `polar` is measured from the +Z
 * (vertical) axis: polar≈0 looks straight down, polar≈π/2 looks toward the
 * horizon. The camera sits on the target's local -Y side (matches
 * `camera.position.set(0, -horiz_dist, vert_dist)` in use_camera.ts) — an
 * arbitrary but fixed choice, since azimuth (not this offset) is what
 * actually varies the apparent viewing direction, by spinning the world
 * underneath it instead.
 */
export function orbit_local_offset(dist: f64, polar: f64): { horiz_dist: f64; vert_dist: f64 } {
  return {
    horiz_dist: dist * Math.sin(polar),
    vert_dist: dist * Math.cos(polar),
  };
}

/**
 * Rotates a 2D vector by `angle` around the Z axis (scene is Z-up — see
 * CAMERA_UP in use_camera.ts): (x cosθ - y sinθ, x sinθ + y cosθ). The one
 * shared primitive behind every azimuth-aware transform in this module tree.
 * Callers pick the sign of `angle` for their own direction of conversion —
 * see this module's doc comment above.
 */
export function rotate2(x: f64, y: f64, angle: f64): { x: f64; y: f64 } {
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  return { x: x * c - y * s, y: x * s + y * c };
}

/**
 * Exponential-decay ease-toward-target factor for one frame of `delta`
 * seconds at rate `k` (bigger k = snappier convergence). Shared by every
 * damped camera/frame value in this module tree (dist/polar in
 * use_camera.ts, azimuth in use_view_frame.ts) so they all converge
 * with the same characteristic curve — apply as
 * `current += (target - current) * damp_alpha(k, delta)`.
 */
export function damp_alpha(k: f64, delta: f64): f64 {
  return 1 - Math.exp(-k * delta);
}
