// @pxl/web/modules/camera/use_camera.ts

import { useEffect, useRef } from "react";
import { invalidate, useFrame, useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";

import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";
import { render_cfg } from "@/modules/world/environment/config.ts";
import { damp_alpha, orbit_local_offset } from "./orbit.ts";

const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
const { CAM_DAMP } = camera_cfg.smooth;

const { FAR_CLIP } = render_cfg;

// frameloop="demand" (scene.tsx) only renders another frame when something
// calls invalidate() — this damping loop is the main thing driving that
// while the camera is settling after a drag/zoom, and needs to stop asking
// for more once it's close enough that a further frame wouldn't look any
// different, or it would invalidate forever and defeat "demand" entirely.
// world units for dist, radians for polar — both small enough that the
// remaining error is sub-pixel at any zoom level in view.
const DIST_EPS: f64 = 0.001;
const POLAR_EPS: f64 = 0.0001;

/** Native Z-up direction for the web world renderer. */
export const CAMERA_UP: V3 = [0, 0, 1];

export function useCamera() {
  const { camera } = useThree();

  /** Applies one-time camera runtime defaults for the Z-up scene. */
  useEffect((): void => {
    camera.up.set(...CAMERA_UP);
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  const view_state = useCameraStore((state) => state.view_state);

  // frameloop="demand": a useFrame callback only runs when a frame is
  // actually being rendered, so on its own this damping loop could never
  // notice view_state changing while the canvas is sitting idle (nothing
  // left invalidating). This effect is what wakes it back up — it fires
  // whenever view_state changes for any reason (drag, wheel, reset_dist/
  // reset_azimuth, ...), independent of whatever the render loop happens to
  // be doing, since React effects run on their own regardless of
  // frameloop mode.
  useEffect(() => {
    invalidate();
  }, [view_state]);

  const did_init = useRef(false);

  const dist = useRef<f64>(view_state.dist);
  const polar = useRef<f64>(view_state.polar);

  useFrame((state, delta) => {
    if (!did_init.current) {
      dist.current = view_state.dist;
      polar.current = view_state.polar;
      did_init.current = true;
    } else {
      const alpha: f64 = damp_alpha(CAM_DAMP, delta);
      dist.current += (view_state.dist - dist.current) * alpha;
      polar.current += (view_state.polar - polar.current) * alpha;
    }

    dist.current = clamp(dist.current, MIN_DIST, MAX_DIST);
    polar.current = clamp(polar.current, MIN_POLAR, MAX_POLAR);

    // still settling — ask for one more frame. See DIST_EPS/POLAR_EPS above.
    if (Math.abs(view_state.dist - dist.current) > DIST_EPS || Math.abs(view_state.polar - polar.current) > POLAR_EPS) {
      state.invalidate();
    }

    // ground-collision clamping used to live here — removed rather than
    // patched again: the ground probe's location is tied to dist/polar
    // themselves, so a small terrain height gets amplified (divided by a
    // near-zero cos(polar)) into a large, noisy correction whenever dist is
    // large or polar is near the horizon. Worth revisiting with a real
    // ray/sphere-cast against the terrain instead of another correction
    // bolted onto orbit math.
    const { horiz_dist, vert_dist } = orbit_local_offset(dist.current, polar.current);

    const pos: V3 = [0, -horiz_dist, vert_dist];

    camera.position.set(pos[0], pos[1], pos[2]);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  });
}
