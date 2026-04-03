// @pxl/web/modules/map/navigation/use-follow-mode.ts

import { useEffect } from "react";

import { useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";

import { useMapStore } from "../store.ts";
import { useMapNavigation } from "./use-map-navigation.ts";

/**
 * Follow Mode(추적 모드) 활성화 시
 * 사용자의 위치와 방향을 지도에 실시간 동기화하는 엔진 훅입니다.
 */
export function useFollowMode() {
  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);

  const heading = useDeviceOrientationStore((state) => state.orientation?.heading ?? null);

  const device_coord = useMapNavigation().device_coord;

  // 1. [위치 동기화] GPS 좌표가 변경될 때만 실행
  useEffect(() => {
    if (!is_follow_mode || device_coord === null) return;
    set_map_center(device_coord);
  }, [is_follow_mode, device_coord]);

  // 2. [시선 동기화] 나침반 각도가 변경될 때만 실행
  useEffect(() => {
    if (!is_follow_mode || heading === null) return;
    set_view_state({ bearing: heading });
  }, [is_follow_mode, heading]);
}
