// @pxl/web/modules/camera/controller.tsx

import * as THREE from "three";
import { OrbitControls } from "@react-three/drei";

import { camera_cfg } from "./config.ts";
import { useCameraKernel } from "./hooks/use-camera-kernel.ts";

const { MIN_DIST, MAX_DIST } = camera_cfg;

export function CameraController() {
  const { controls_ref, on_interaction_start, on_interaction_end, on_interaction_change } = useCameraKernel();

  return (
    <OrbitControls
      ref={controls_ref}
      makeDefault
      mouseButtons={{
        LEFT: THREE.MOUSE.PAN,
        MIDDLE: THREE.MOUSE.DOLLY,
        RIGHT: THREE.MOUSE.ROTATE,
      }}
      touches={{
        ONE: THREE.TOUCH.PAN,
        TWO: THREE.TOUCH.DOLLY_ROTATE,
      }}
      enableRotate
      rotateSpeed={1}
      enableDamping
      dampingFactor={0.08}
      minDistance={MIN_DIST}
      maxDistance={MAX_DIST}
      screenSpacePanning={false}
      maxPolarAngle={Math.PI / 2.1}
      minPolarAngle={0}
      onStart={on_interaction_start}
      onChange={on_interaction_change}
      onEnd={on_interaction_end}
    />
  );
}
