// @pxl/web/module/camera/use_camera.ts

import { useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";

import { useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";
import { damp_alpha } from "./orbit.ts";
import { climb_polar } from "./terrain_gate.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { request_world_tick as invalidate } from "@/module/world/change.ts";
import { view, view_compose } from "@/module/world/frame/view.ts";

const { MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;
const { CAM_DAMP } = camera_cfg.smooth;

// The tick runs another frame only when something asks (sched/tick.ts) —
// this damping loop is the main thing asking while the camera is settling
// after a drag/zoom, and needs to stop once it's close enough that a
// further frame wouldn't look any different, or it would ask forever and
// the view would never be idle.
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

/** Damps the eye's distance and pitch toward the controls and writes them
 * into the view (module/world/frame/view.ts). */
export function useCamera() {
  const view_state = useCameraStore((state) => state.view_state);

  // A step only runs when a frame is asked for, so on its own this damping
  // loop could never notice view_state changing while the canvas is sitting
  // idle. This effect is what wakes it back up — it fires whenever view_state
  // changes for any reason (drag, wheel, reset_dist/reset_azimuth, ...),
  // independent of whatever the frame loop happens to be doing.
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

  useTick("interaction", ({ state, delta, view_moving }) => {
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
      view_moving();
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
    // use_view_frame.ts. Outside climb mode the offset is zero and the
    // drawn pitch is exactly the damped control.
    const want_climb: f64 = useSettingsStore.getState().camera_terrain === "climb" ? climb_polar(dist.current, polar.current, view_state.azimuth) - polar.current : 0;
    const climb_k = want_climb < shown_climb.current ? CLIMB_RISE_DAMP : CLIMB_RELAX_DAMP;
    shown_climb.current += (want_climb - shown_climb.current) * damp_alpha(climb_k, delta);
    if (Math.abs(want_climb - shown_climb.current) > CLIMB_EPS) {
      state.invalidate();
      view_moving();
    }

    // The offset lags the control, so a pitch steepening toward vertical can
    // still be carrying a climb from a shallower one.
    const view_polar = clamp(polar.current + shown_climb.current, MIN_POLAR, MAX_POLAR);
    if (view.dist !== dist.current || view.polar !== view_polar) {
      view.dist = dist.current;
      view.polar = view_polar;
      view_compose();
    }
  }, "camera");
}
