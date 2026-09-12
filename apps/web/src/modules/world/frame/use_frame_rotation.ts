// @pxl/web/modules/world/frame/use_frame_rotation.ts

import { type RefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { damp_alpha } from "@/modules/camera/orbit.ts";
import { useTick } from "@/modules/world/sched/tick.ts";

const { ROT_DAMP } = camera_cfg.smooth;

// see use_camera.ts's DIST_EPS/POLAR_EPS comment — same frameloop="demand" reasoning.
const AZIMUTH_EPS: f64 = 0.0001;

type UseFrameRotationProps = {
  frame_ref: RefObject<THREE.Group | null>;
};
export function useFrameRotation(props: UseFrameRotationProps) {
  const { frame_ref } = props;

  const azimuth = useCameraStore((state) => state.view_state.azimuth);
  const did_init = useRef(false);

  // wakes the loop back up when azimuth changes while the canvas is idle —
  // see use_camera.ts's matching effect for why this can't just rely on the
  // useFrame below noticing on its own under frameloop="demand".
  useEffect(() => {
    invalidate();
  }, [azimuth]);

  useTick("interaction", ({ state, delta }) => {
    const frame = frame_ref.current;
    if (!frame) return;

    if (!did_init.current) {
      frame.rotation.z = azimuth;
      frame.updateMatrix();
      did_init.current = true;
      return;
    }

    const alpha: f64 = damp_alpha(ROT_DAMP, delta);
    const err = wrap(azimuth - frame.rotation.z, -Math.PI, Math.PI);
    frame.rotation.z += err * alpha;
    frame.updateMatrix();

    if (Math.abs(err * (1 - alpha)) > AZIMUTH_EPS) state.invalidate();
  });
}
