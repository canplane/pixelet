// @pxl/web/modules/world/camera/camera-controller.tsx

import * as THREE from "three";
import { OrbitControls } from "@react-three/drei";

import { CameraControlMode, render_cfg } from "@/modules/world/render/config.ts";
import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useCameraKernel } from "./hooks/use-camera-kernel.ts";

const { MIN_DIST, MAX_DIST } = render_cfg.camera;

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
      // 시작: 무조건 USER 모드로 잠금
      onStart={(e) => {
        set_camera_control_mode(CameraControlMode.USER); // 🔒 Lock: 외부(Zustand) 명령 차단
        on_interaction_start();
      }}
      // 끝: 현재가 USER 모드일 때만 해제 (SYSTEM 모드라면 건드리지 않음)
      onEnd={(e) => {
        on_interaction_end();
        if (camera_control_mode === CameraControlMode.USER) {
          set_camera_control_mode(CameraControlMode.NONE); // 🔓 Unlock: 다시 외부 명령(FlyTo 등) 수용 가능 상태로 전환
        }
      }}
    />
  );
}
