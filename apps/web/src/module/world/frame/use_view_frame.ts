// @pxl/web/module/world/frame/use_view_frame.ts

/**
 * The root frame's two damped halves (module/world/frame/view.ts): its turn
 * about Z toward the controls' azimuth, and its translation toward putting
 * the map centre at the origin — the view's origin rebased in the same
 * step the translation snaps, so nothing reads the two apart for a tick.
 */

import { useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";
import { fvec, type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useCameraStore } from "@/module/camera/store.ts";
import { camera_cfg } from "@/module/camera/config.ts";
import { damp_alpha } from "@/module/camera/orbit.ts";
import { request_world_tick as invalidate } from "@/module/world/change.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { view, view_compose } from "@/module/world/frame/view.ts";

const { ROT_DAMP, POS_DAMP } = camera_cfg.smooth;

// see use_camera.ts's DIST_EPS/POLAR_EPS comment — the same reasoning about
// asking for frames.
const AZIMUTH_EPS: f64 = 0.0001;
const POS_EPS: f64 = 0.0005;

export function useViewRotation(): void {
  const azimuth = useCameraStore((state) => state.view_state.azimuth);
  const did_init = useRef(false);

  // wakes the loop back up when azimuth changes while the canvas is idle —
  // see use_camera.ts's matching effect for why this can't just rely on the
  // step below noticing on its own.
  useEffect(() => {
    invalidate();
  }, [azimuth]);

  useTick("interaction", ({ state, delta, view_moving }) => {
    if (!did_init.current) {
      view.azimuth = azimuth;
      view_compose();
      did_init.current = true;
      return;
    }

    const alpha: f64 = damp_alpha(ROT_DAMP, delta);
    const err = wrap(azimuth - view.azimuth, -Math.PI, Math.PI);
    if (err === 0) return;
    view.azimuth += err * alpha;
    view_compose();

    if (Math.abs(err * (1 - alpha)) > AZIMUTH_EPS) {
      state.invalidate();
      view_moving();
    }
  }, "frame_rotation");
}

type UseViewTranslationProps = {
  map_center: PVF3 | null;
  map_origin: IVec;
};

export function useViewTranslation(props: UseViewTranslationProps): void {
  const { map_center, map_origin } = props;
  const prev_map_origin = useRef<IVec>(map_origin);
  const did_init = useRef(false);

  // wakes the loop back up when the damping target moves (GPS update,
  // follow-mode re-center, floating-origin rebase, ...) while the canvas is
  // idle — see use_camera.ts's matching effect.
  useEffect(() => {
    invalidate();
  }, [map_center, map_origin]);

  useTick("interaction", ({ state, delta, view_moving }) => {
    if (map_center === null) return;

    const did_rebase: boolean = prev_map_origin.current !== map_origin;
    prev_map_origin.current = map_origin;

    if (did_rebase) view.origin = map_origin;

    const local_i: IVec = ivec.sub(map_center.i, map_origin);

    const tx: f64 = -(ivec.to_real(ivec.X(local_i)) + fvec.to_real(fvec.X(map_center.f)));
    const ty: f64 = -(ivec.to_real(ivec.Y(local_i)) + fvec.to_real(fvec.Y(map_center.f)));
    const tz: f64 = -(ivec.to_real(ivec.Z(local_i)) + fvec.to_real(fvec.Z(map_center.f)));

    if (!did_init.current || did_rebase) {
      did_init.current = true;
      view.origin = map_origin;
      view.tx = tx;
      view.ty = ty;
      view.tz = tz;
      view_compose();
      view_moving();
      return;
    }

    if (tx === view.tx && ty === view.ty && tz === view.tz) return;
    const alpha: f64 = 1 - Math.exp(-POS_DAMP * delta);
    view.tx += (tx - view.tx) * alpha;
    view.ty += (ty - view.ty) * alpha;
    view.tz += (tz - view.tz) * alpha;
    view_compose();

    if (Math.abs(tx - view.tx) > POS_EPS || Math.abs(ty - view.ty) > POS_EPS || Math.abs(tz - view.tz) > POS_EPS) {
      state.invalidate();
      view_moving();
    }
  }, "frame_translation");
}
