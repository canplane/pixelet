// @pxl/web/modules/world/camera/hooks/use-camera-kernel.ts

import { useRef } from "react";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import { ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";
import { from3 } from "@pxl/spec/space/adapter/three.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { useCameraInit } from "./use-camera-init.ts";
import { useCameraFlight } from "./use-camera-flight.ts";
import { useCameraFrame } from "./use-camera-frame.ts";
import { useCameraGroundAlign } from "./use-camera-ground-align.ts";

/**
 * 🧠 [useCameraKernel]
 * 분산된 카메라 기동/보정 훅들을 하나로 통합하여 내부 상태(flyState 등)를 캡슐화함.
 */
export function useCameraKernel() {
  const controls_ref = useRef<OrbitControlsImpl>(null);

  const set_map_center = useMapStore.getState().set_map_center;
  const set_is_flying = useMapStore.getState().set_is_flying;
  const set_is_follow_mode = useMapStore.getState().set_is_follow_mode;

  const map_origin = useMapNavigation().map_origin;

  // 초기화 로직
  useCameraInit();

  // ✈️ 비행 상태를 커널이 소유
  // 이동 및 동기화, 시그널 처리 로직 (flyState 반환)
  const { fly_target } = useCameraFlight({ controls_ref });

  // 지형 밀착
  useCameraGroundAlign({ controls_ref, fly_target });

  // 프레임 루프 로직 (애니메이션)
  useCameraFrame({ controls_ref, fly_target });

  /** 핸들러 분리: 커널 제어권 전환 로직 */
  const on_interaction_start = () => {
    // 부수 효과 처리
    set_is_follow_mode(false);

    // 실행 중인 비행 강제 종료 (이미 useCameraFlight 내부에 있지만 더블 가드)
    set_is_flying(false);
  };

  const on_interaction_end = () => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { target } = controls;
    const yup: V3 = [target.x, target.y, target.z];

    // 수치 유효성 검증
    if (!yup.every(Number.isFinite)) return;

    const center: PVF3 = projection.project({ i: 0n, f: 0n }, ...from3(yup));
    center.i = ivec.add(center.i, map_origin);

    set_map_center(center);
  };

  return { controls_ref, on_interaction_start, on_interaction_end };
}
