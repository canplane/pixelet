// @pxl/web/modules/world/camera/hooks/use-camera-flight.ts

import { useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { pvf3 } from "@pxl/spec/space/pvf3/mod.ts";

import { CameraControlMode, render_cfg } from "@/modules/world/render/config.ts";
import { derive_camera_pose } from "@pxl/spec/space/adapter/three.ts";
import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";

const { DEFAULT_DIST, MIN_DIST, DEFAULT_POLAR_ANGLE } = render_cfg.camera;

const TRAVEL_DIST_THRES = 250; // 임계값 (예: 1km)

export type CameraFlyTarget = {
  dst_pos: THREE.Vector3;
  dst_look_at: THREE.Vector3;
};

type UseCameraFlightProps = {
  controls_ref: React.RefObject<OrbitControlsImpl | null>;
};
export function useCameraFlight(props: UseCameraFlightProps) {
  const { controls_ref } = props;

  const map_center = useMapStore((state) => state.map_center);
  const view_state = useMapStore((state) => state.view_state);
  const set_view_state = useMapStore.getState().set_view_state;
  const camera_control_mode = useMapStore((state) => state.camera_control_mode);
  const set_is_flying = useMapStore.getState().set_is_flying;
  const reset_signal = useMapStore((state) => state.camera_reset_signal);

  const map_origin = useMapNavigation().map_origin;

  const { camera } = useThree();

  /** 🏁 [Arrived-Checkpoint]
   * 카메라가 이미 도달하여 정착(Settle)된 지점의 식별자입니다.
   * 불필요한 재진입을 막는 '이미 도착함'의 지표입니다.
   */
  const arrived_key = useRef<string>("");

  /** 🗺️ [Flight-Plan]
   * 'To-be Arrived' - 도달해야 할 목적지와 비행 계획입니다.
   * 실제 보간 실행은 useFrame(useCameraFrame)에서 수행.
   */
  const fly_target = useRef<CameraFlyTarget>({
    dst_pos: new THREE.Vector3(), // 카메라가 도달할 최종 물리 위치
    dst_look_at: new THREE.Vector3(), // 카메라가 바라볼 최종 지점 (Center의 Local 변환값)
  });

  /**
   * ✈️ [Flight-Planning-Engine]
   * 모든 카메라 기동(Maneuver)의 최종 목적지와 방식을 결정하는 단일 창구(Entry Point).
   */
  const plan_flight = (dst_pos: THREE.Vector3, dst_look_at: THREE.Vector3, opts: { snap?: boolean } = {}) => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { snap = false } = opts;

    // [Execution Strategy] 이동 거리에 따라 Snap(즉시) 또는 Fly(보간) 집행
    if (snap) {
      camera.position.copy(dst_pos);
      controls.target.copy(dst_look_at);
      controls.update(); // 렌더링 프레임 대기 없이 즉시 행렬 갱신
      set_is_flying(false);
    } else {
      fly_target.current.dst_pos.copy(dst_pos);
      fly_target.current.dst_look_at.copy(dst_look_at);
      set_is_flying(true);
    }
  };

  /** 🔔 [Signal-Listeners] 데이터 수신 및 계획 수립부 */

  // 1. [Center Update] 외부 좌표 변화에 따른 비행 계획
  useEffect((): void => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!map_center || !controls) {
      // 🔒 [Mutex Interrupt] 사용자 조작이 개입되면 진행 중인 모든 비행 즉시 취소
      if (camera_control_mode === CameraControlMode.USER) set_is_flying(false);
      return;
    }

    if (camera_control_mode === CameraControlMode.USER) return;

    // 🎯 [Destination-Check] 'Arrived'와 'Bound'의 대조
    const bound_key = [
      pvf3.repr(map_center),
      view_state.dist,
      view_state.bearing,
      view_state.polar_angle,
    ].join("|");
    if (arrived_key.current === bound_key) return;

    const dist: f64 = view_state.dist > MIN_DIST ? view_state.dist : DEFAULT_DIST;
    const polar_angle: f64 = view_state.polar_angle || DEFAULT_POLAR_ANGLE;

    const { look_at, pos } = derive_camera_pose(map_center, map_origin, dist, view_state.bearing, polar_angle);

    const dst_look_at = new THREE.Vector3(...look_at);
    const dst_pos = new THREE.Vector3(...pos);
    const travel_dist: f64 = controls.target.distanceTo(dst_look_at);

    // 비행 계획 수립 (Snap vs Fly)
    plan_flight(dst_pos, dst_look_at, { snap: travel_dist > TRAVEL_DIST_THRES });

    // 이제 이 좌표는 'Arrived'로 간주하여 중복 진입 방지
    arrived_key.current = bound_key;
  }, [
    map_center,
    camera_control_mode,
    view_state.dist,
    view_state.bearing,
    view_state.polar_angle,
  ]);

  // 2. [Distance Reset] 고도(Zoom) 기본값 복구 시그널 처리
  useEffect(() => {
    if (reset_signal.dist === 0) return;
    set_view_state({ dist: DEFAULT_DIST });
  }, [reset_signal.dist]);

  // 3. [Orientation Reset] 수직각(Polar) 기본값 복구 시그널 처리
  useEffect(() => {
    if (reset_signal.orientation === 0) return;
    set_view_state({ polar_angle: DEFAULT_POLAR_ANGLE });
  }, [reset_signal.orientation]);

  return { fly_target };
}
