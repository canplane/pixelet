// @pxl/web/modules/world/frame/use_frame_rotation.ts

import { type RefObject, useRef } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { damp_alpha } from "@/modules/camera/orbit.ts";

const { ROT_DAMP } = camera_cfg.smooth;

type UseFrameRotationProps = {
  frame_ref: RefObject<THREE.Group | null>;
};
export function useFrameRotation(props: UseFrameRotationProps) {
  const { frame_ref } = props;

  const azimuth = useCameraStore((state) => state.view_state.azimuth);
  const did_init = useRef(false);

  useFrame((_, delta) => {
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
  });
}
