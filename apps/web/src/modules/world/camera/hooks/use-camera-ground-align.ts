// @pxl/web/modules/world/camera/hooks/use-camera-ground-align.ts

import { type RefObject, useLayoutEffect } from "react";
import { useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { ivec } from "@pxl/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import type { CameraFlyTarget } from "./use-camera-flight.ts";
import { get_ele } from "@/modules/world/terrain/use-elevation.ts";

/**
 * 🎯 [Center-Reactive-Align]
 * useFrame 없이 center가 변경될 때만 해당 좌표의 지형 높이를 조회하여
 * 카메라와 타겟의 고도(Y)를 보정합니다.
 */

type UseCameraGroundAlignProps = {
  controls_ref: React.RefObject<OrbitControlsImpl | null>;
  fly_target: RefObject<CameraFlyTarget>;
};
export function useCameraGroundAlign(props: UseCameraGroundAlignProps) {
  const { controls_ref, fly_target } = props;

  const map_center = useMapStore((state) => state.map_center);
  const is_flying = useMapStore((state) => state.is_flying);

  const map_origin = useMapNavigation().map_origin;

  const { camera } = useThree();

  useLayoutEffect(() => {
    const controls = controls_ref.current;
    if (!controls || map_center === null) return;

    // 1. 현재 전역 좌표(center)의 실제 지형 높이 조회
    const center_ele: f64 = get_ele(map_center.i, map_center.f) ?? 0;
    // const center_abs_z: f64 = pvf3.to_real(pvf3.Z(center));

    // 2. Three.js 로컬 Y 좌표 계산 (L4 anchor Z 보정)
    const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
    const target_yup_y: f64 = center_ele - origin_z;

    // 3. 현재 타겟 높이와 지형 높이 비교
    const current_yup_y: f64 = controls.target.y;

    // 델타값 계산 (지형 변화량)
    const delta_yup_y: f64 = target_yup_y - current_yup_y;

    // console.log("ground-align", {
    //   center_abs_z,
    //   center_ele,
    //   origin_z,
    //   target_yup_y,
    //   current_yup_y,
    //   delta_yup_y,
    // });

    if (Math.abs(delta_yup_y) < 0.01) return;

    if (is_flying) {
      /**
       * ✈️ [Flight-Correction]
       * 비행 중이라면 현재 물리 위치를 건드리지 않고,
       * 도착지(dst)의 높이만 지형에 맞춰 수정합니다.
       * 이렇게 하면 보간이 자연스럽게 지형 위로 착륙합니다.
       */
      fly_target.current.dst_look_at.y = target_yup_y;
      fly_target.current.dst_pos.y += delta_yup_y;
    } else {
      /**
       * ⚓ [Static-Correction]
       * 정지 상태(또는 Snap)일 때는 기존처럼 즉시 이동
       */
      camera.position.y += delta_yup_y;
      controls.target.y = target_yup_y;

      // 행렬 갱신
      controls.update();
    }
  }, [map_center, map_origin, camera, fly_target, is_flying, controls_ref]);
}
