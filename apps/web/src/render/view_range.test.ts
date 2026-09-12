// @pxl/web/render/view_range.test.ts

/**
 * The invariant the render range rests on.
 *
 * Ranges are measured from the view centre on the ground (one anchor, chunk
 * aligned — see module/world/frame/use_range_anchor.ts), while the camera
 * itself can stand up to MAX_DIST away from that centre. That is only safe
 * because the camera always looks at the centre, which keeps everything it
 * can see near it. This measures that claim directly: cast the frustum at the
 * ground over every zoom, tilt, aspect ratio and pivot height the camera
 * allows, and check that ground the camera can actually see — inside the
 * frustum and nearer than fog — always lies within the distance chunks are
 * mounted to.
 *
 * If this fails, the range is not the thing to nudge first: it means the
 * camera model changed (a wider field of view, a pivot the camera no longer
 * looks at, a longer leash) and the anchor has to change with it.
 */

import { assertEquals } from "@std/assert";

import { camera_cfg } from "@/module/camera/config.ts";
import { render_cfg } from "@/module/world/environment/config.ts";

const { FOV } = camera_cfg.projection;
const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;

/** How far the chunk-aligned anchor can sit from the true view centre. */
const ANCHOR_QUANTIZATION = Math.hypot(64, 64);

/**
 * The farthest visible ground point from the view centre, in world units.
 *
 * `pivot_height` is how far the centre stands above the terrain — a user
 * placed object floats, and the camera rises with whatever it is looking at,
 * which is the case that reaches furthest.
 */
function farthest_visible_ground(aspect: number, pivot_height: number, steps = 12): number {
  const vh = (FOV * Math.PI) / 360;
  const hh = Math.atan(Math.tan(vh) * aspect);
  let worst = 0;

  for (let di = 0; di <= steps; di++) {
    const dist = MIN_DIST + ((MAX_DIST - MIN_DIST) * di) / steps;
    for (let pi = 0; pi <= steps; pi++) {
      const polar = MIN_POLAR + ((MAX_POLAR - MIN_POLAR) * pi) / steps;
      // The camera orbits the centre: horizontally behind it, vertically above
      // it, always looking straight at it (module/camera/orbit.ts).
      const horiz = dist * Math.sin(polar);
      const height = pivot_height + dist * Math.cos(polar);
      // Forward is the unit vector from the camera to the centre; right is +X
      // because the camera carries no azimuth of its own (the world rotates
      // underneath it), and up completes the basis.
      const len = Math.hypot(horiz, dist * Math.cos(polar)) || 1;
      const f = [0, horiz / len, (-dist * Math.cos(polar)) / len];
      const r = [1, 0, 0];
      const u = [f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0]];

      for (let yi = -steps; yi <= steps; yi++) {
        const ty = Math.tan((hh * yi) / steps);
        for (let qi = -steps; qi <= steps; qi++) {
          const tp = Math.tan((vh * qi) / steps);
          const d = [f[0] + ty * r[0] + tp * u[0], f[1] + ty * r[1] + tp * u[1], f[2] + ty * r[2] + tp * u[2]];
          const dl = Math.hypot(d[0], d[1], d[2]);
          const dz = d[2] / dl;
          if (dz >= 0) continue; // looking at or above the horizon: no ground
          const t = height / -dz;
          if (t <= 0 || t > render_cfg.FOG_FAR) continue; // fog hides it anyway
          const x = t * (d[0] / dl);
          const y = -horiz + t * (d[1] / dl);
          worst = Math.max(worst, Math.hypot(x, y));
        }
      }
    }
  }
  return worst;
}

Deno.test("view range - everything the camera can see is inside the distance chunks are mounted to", () => {
  // Aspect ratios from a portrait phone to an ultrawide desktop, and pivot
  // heights covering ground, a placed object standing above it, and a tall
  // structure.
  for (const aspect of [0.5, 1, 16 / 9, 2.5, 3]) {
    for (const pivot_height of [0, 50, 100, 200, 400]) {
      const reach = farthest_visible_ground(aspect, pivot_height) + ANCHOR_QUANTIZATION;
      assertEquals(
        reach <= render_cfg.CHUNK_RENDER_DIST,
        true,
        `aspect ${aspect}, pivot +${pivot_height}u: visible ground reaches ${Math.round(reach)}u, mounted to ${render_cfg.CHUNK_RENDER_DIST}u`,
      );
    }
  }
});

Deno.test("view range - fog finishes before geometry does, at every pivot height", () => {
  // The other half of the same contract: the range exists so that fog, not a
  // missing chunk, is what ends the picture.
  assertEquals(render_cfg.FOG_FAR <= render_cfg.CHUNK_RENDER_DIST, true);
  assertEquals(render_cfg.FOG_NEAR < render_cfg.FOG_FAR, true);
});
