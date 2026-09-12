// @pxl/web/modules/world/frame/use_frame_translation.ts

import { type RefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";

import { fvec, type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { camera_cfg } from "@/modules/camera/config.ts";
import type { AnchorRegistry } from "@/modules/world/anchor/registry.ts";
import { useTick } from "@/modules/world/sched/tick.ts";

const { POS_DAMP } = camera_cfg.smooth;

// see use_camera.ts's DIST_EPS/POLAR_EPS comment — same frameloop="demand" reasoning.
const POS_EPS: f64 = 0.0005;

type UseFrameTranslationProps = {
  frame_ref: RefObject<THREE.Group | null>;

  map_center: PVF3 | null;
  map_origin: IVec;

  /**
   * Anchor registry to rebase alongside the frame, in the same callback —
   * keeps the origin rebase and the translation update as one atomic step
   * instead of two independently-scheduled per-frame subscriptions, which
   * could otherwise leave anchors and the local-world-frame disagreeing
   * about the origin for a tick (a stray offset/duplicate look).
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

  useTick("interaction", ({ state, delta, view_moving }) => {
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
      // The origin just moved by a whole L4 (4096u) and every anchor group
      // moved with it, but only local matrices have been written: three
      // recomputes matrixWorld during render, which is after this tick. The
      // visible phase reads chunk world matrices to decide what is on screen,
      // so without this every chunk would measure 4096u from where it really
      // is for one frame — off screen, and an off-screen chunk gives its mesh
      // back, which empties the world and then rebuilds it. One extra matrix
      // walk, only on the rare frame the origin actually moves.
      state.scene.updateMatrixWorld(true);
      view_moving();
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
      view_moving();
    }
  });
}
