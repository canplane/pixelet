// @pxl/web/modules/world/frame/use_frame_translation.ts

import { type RefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { invalidate, useFrame } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";

import { fvec, type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { camera_cfg } from "@/modules/camera/config.ts";
import type { AnchorRegistry } from "@/modules/world/spatial/anchor/registry.ts";

const { POS_DAMP } = camera_cfg.smooth;

// see use_camera.ts's DIST_EPS/POLAR_EPS comment — same frameloop="demand" reasoning.
const POS_EPS: f64 = 0.0005;

type UseFrameTranslationProps = {
  frame_ref: RefObject<THREE.Group | null>;

  map_center: PVF3 | null;
  map_origin: IVec;

  /**
   * Anchor registry to rebase alongside the frame, in the same callback.
   * These used to be two independent useFrame subscriptions reacting to the
   * same map_origin change — nothing guaranteed which ran first, or that
   * both landed within the same tick, so a rebase could show anchors and
   * the local-world-frame briefly disagreeing about the origin (a stray
   * offset/duplicate look). Folding the rebase in here makes the two
   * updates one atomic step instead of two independently-scheduled ones.
   */
  registry?: AnchorRegistry;
};
export function useFrameTranslation(props: UseFrameTranslationProps) {
  const { frame_ref, map_center, map_origin, registry } = props;
  const prev_map_origin = useRef<IVec>(map_origin);
  const did_init = useRef(false);

  // wakes the loop back up when the damping target moves (GPS update,
  // follow-mode re-center, floating-origin rebase, ...) while the canvas is
  // idle — see use_camera.ts's matching effect for why the useFrame below
  // can't notice this on its own under frameloop="demand".
  useEffect(() => {
    invalidate();
  }, [map_center, map_origin]);

  useFrame((state, delta) => {
    const frame = frame_ref.current;
    if (!frame || map_center === null) return;

    const did_rebase: boolean = prev_map_origin.current !== map_origin;
    prev_map_origin.current = map_origin;

    if (did_rebase && registry && registry.cur_map_origin !== map_origin) {
      registry.shift_map_origin(map_origin);
    }

    const local_i: IVec = ivec.sub(map_center.i, map_origin);

    const tx: f64 = -(ivec.to_real(ivec.X(local_i)) + fvec.to_real(fvec.X(map_center.f)));
    const ty: f64 = -(ivec.to_real(ivec.Y(local_i)) + fvec.to_real(fvec.Y(map_center.f)));
    const tz: f64 = -(ivec.to_real(ivec.Z(local_i)) + fvec.to_real(fvec.Z(map_center.f)));

    if (!did_init.current || did_rebase) {
      did_init.current = true;
      frame.position.set(tx, ty, tz);
      frame.updateMatrix();
      return;
    }

    const alpha: f64 = 1 - Math.exp(-POS_DAMP * delta);
    frame.position.x += (tx - frame.position.x) * alpha;
    frame.position.y += (ty - frame.position.y) * alpha;
    frame.position.z += (tz - frame.position.z) * alpha;
    frame.updateMatrix();

    if (
      Math.abs(tx - frame.position.x) > POS_EPS ||
      Math.abs(ty - frame.position.y) > POS_EPS ||
      Math.abs(tz - frame.position.z) > POS_EPS
    ) {
      state.invalidate();
    }
  });
}
