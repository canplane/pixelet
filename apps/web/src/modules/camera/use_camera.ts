// @pxl/web/modules/camera/use_camera.ts

import { useEffect, useRef } from "react";
import { invalidate, useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";

import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";
import { render_cfg } from "@/modules/world/environment/config.ts";
import { damp_alpha, orbit_local_offset } from "./orbit.ts";
import { climb_polar } from "./terrain_gate.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useTick } from "@/modules/world/sched/tick.ts";

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

/**
 * How quickly the climb's own contribution follows what terrain asks for.
 *
 * Elevation is read per cell, so the answer steps rather than slides.
 * Rising is the faster of the two: late to rise means the eye is briefly
 * inside a hill, late to relax means the view sits steeper than it needs to
 * for a moment longer.
 */
const CLIMB_RISE_DAMP: f64 = 20;
const CLIMB_RELAX_DAMP: f64 = 5;
const CLIMB_EPS: f64 = 0.0005;

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
  /** How much steeper than the control the drawn pitch currently is, always
   * <= 0. Zero unless terrain is being climbed over; view_state.polar is
   * never touched by it. */
  const shown_climb = useRef<f64>(0);

  useTick("interaction", ({ state, delta }) => {
    if (!did_init.current) {
      dist.current = view_state.dist;
      polar.current = view_state.polar;
      shown_climb.current = 0;
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

    // Ground collision used to be a clamp here that corrected `polar` from a
    // probe whose own location depends on polar, so a small terrain height
    // came back divided by a near-zero cos(polar). Both modes that replaced
    // it avoid that loop: `stop` refuses the step in the store and touches
    // nothing here, and the climb below moves an angle the probe does not
    // read.
    //
    // `climb` grants the control and answers terrain by pitching the view
    // up instead. dist stays exactly what the scale bar reads; only the
    // angle gives way, and it comes back on its own once the ground does.
    // What is smoothed here is the climb's own contribution, never the pitch
    // itself. polar is already damped alongside dist just above, and running
    // the absolute angle through a second lag gave tilt an inertia rotation
    // does not have — rotation gets one lag, at the same rate, in
    // use_frame_rotation.ts. Outside climb mode the offset is zero and the
    // drawn pitch is exactly the damped control.
    const want_climb: f64 = useSettingsStore.getState().camera_terrain === "climb" ? climb_polar(dist.current, polar.current, view_state.azimuth) - polar.current : 0;
    const climb_k = want_climb < shown_climb.current ? CLIMB_RISE_DAMP : CLIMB_RELAX_DAMP;
    shown_climb.current += (want_climb - shown_climb.current) * damp_alpha(climb_k, delta);
    if (Math.abs(want_climb - shown_climb.current) > CLIMB_EPS) state.invalidate();

    // The offset lags the control, so a pitch steepening toward vertical can
    // still be carrying a climb from a shallower one.
    const view_polar = clamp(polar.current + shown_climb.current, MIN_POLAR, MAX_POLAR);
    const { horiz_dist, vert_dist } = orbit_local_offset(dist.current, view_polar);

    const pos: V3 = [0, -horiz_dist, vert_dist];

    camera.position.set(pos[0], pos[1], pos[2]);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  });
}
