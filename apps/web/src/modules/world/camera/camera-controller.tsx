// @pxl/web/modules/world/camera/camera-controller.tsx

import * as THREE from "three";
import { OrbitControls } from "@react-three/drei";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { camera_cfg, CameraControlMode } from "@/modules/world/render/config.ts";
import { useCameraKernel } from "./hooks/use-camera-kernel.ts";

const { MIN_DIST, MAX_DIST } = camera_cfg;

export function CameraController() {
  const camera_control_mode = useMapStore((state) => state.camera_control_mode);
  const set_camera_control_mode = useMapStore.getState().set_camera_control_mode;

  const { controls_ref, on_interaction_start, on_interaction_end } = useCameraKernel();

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
      onStart={() => {
        on_interaction_start();
      }}
      onEnd={() => {
        on_interaction_end();
        if (camera_control_mode === CameraControlMode.USER) {
          set_camera_control_mode(CameraControlMode.NONE);
        }
      }}
    />
  );
}
