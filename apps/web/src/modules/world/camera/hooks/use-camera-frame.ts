// @pxl/web/modules/world/camera/hooks/use-camera-frame.ts

import { type RefObject, useLayoutEffect, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import type { CameraFlyTarget } from "./use-camera-flight.ts";
import { CameraControlMode } from "../../render/config.ts";

type UseCameraFrameProps = {
  controls_ref: React.RefObject<OrbitControlsImpl | null>;
  fly_target: RefObject<CameraFlyTarget>;
};
export function useCameraFrame(props: UseCameraFrameProps) {
  const { controls_ref, fly_target } = props;

  const camera_control_mode = useMapStore((state) => state.camera_control_mode);
  const set_view_state = useMapStore.getState().set_view_state;
  const is_flying = useMapStore((state) => state.is_flying);
  const set_is_flying = useMapStore.getState().set_is_flying;

  const { camera } = useThree();

  const last_update = useRef({ dist: 0, bearing: 0 });
  const last_update_time = useRef(0);

  // 1. 📊 [Reporting] 물리 엔진의 상태를 스토어에 동기화 (Event-driven)
  useLayoutEffect(() => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const handleReport = () => {
      // 애니메이션 중일 때는 useFrame이 update를 호출하므로 여기서 중복 보고를 막기 위해 throttle 적용 가능
      const now: f64 = performance.now();
      if (now - last_update_time.current < 50) return; // 50ms (20fps 수준)로 UI 보고 제한

      const dist: f64 = camera.position.distanceTo(controls.target);
      const height: f64 = camera.position.y - controls.target.y;
      const polar_angle: f64 = controls.getPolarAngle();
      let bearing: f64 = -controls.getAzimuthalAngle() * (180 / Math.PI);

      // Bearing 정규화 (-180 ~ 180)
      if (bearing <= -180) bearing += 360;
      if (bearing > 180) bearing -= 360;

      // 🎯 [Threshold Guard] 유의미한 변화일 때만 스토어 전파
      const is_changed: boolean = Math.abs(last_update.current.dist - dist) > Math.max(dist * 0.005, 0.001) ||
        Math.abs(last_update.current.bearing - bearing) > 0.1;

      if (is_changed) {
        last_update.current = { dist, bearing };
        last_update_time.current = now;
        set_view_state({ dist, height, bearing, polar_angle });
      }
    };

    controls.addEventListener("change", handleReport);
    return () => controls.removeEventListener("change", handleReport);
  }, [controls_ref, camera, set_view_state]);

  // 2. 🚀 [Animation] flyState 계획 집행 (Exponential Interpolation)
  useFrame((_, delta) => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls || !is_flying) return;

    // 🔒 [Interrupt] 사용자 개입 시 비행 즉시 취소
    if (camera_control_mode === CameraControlMode.USER) {
      set_is_flying(false);
      return;
    }

    const { dst_pos, dst_look_at } = fly_target.current;

    // 지수적 보간: delta를 활용해 프레임 독립적인 smoothing factor 적용
    const alpha: f64 = 1 - Math.pow(0.001, delta);

    camera.position.lerp(dst_pos, alpha);
    controls.target.lerp(dst_look_at, alpha);

    // controls.update()가 내부적으로 'change' 이벤트를 발생시켜 위 Reporting 로직을 실행함
    controls.update();

    // 🏁 [Termination] 수렴 조건 확인
    if (camera.position.distanceTo(dst_pos) < 0.05) {
      camera.position.copy(dst_pos);
      controls.target.copy(dst_look_at);
      set_is_flying(false);
    }
  });
}
