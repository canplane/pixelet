import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useThree } from '@react-three/fiber';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { toThreeCoord } from '@/shared/lib/three/three-coords';
import { useCoordinateConverter } from '@/entities/osm/lib/use-coordinate-converter';
import { useMapStore } from '@/entities/osm/model/map-store';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';

const { DEFAULT_DIST, MIN_DISTANCE, DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

const TRAVEL_DIST_THRES = 1000; // 임계값 (예: 1km)

export interface CameraFlyTarget {
  dstPos: THREE.Vector3;
  dstLookAt: THREE.Vector3;
}

interface UseCameraMovementProps {
  controlsRef: React.RefObject<OrbitControlsImpl | null>;
}

export function useCameraMovement({ controlsRef }: UseCameraMovementProps) {
  const { cameraControlMode, center, setIsFlying } = useMapNavigation();
  const resetSignal = useMapStore((state) => state.cameraResetSignal);

  const { camera } = useThree();
  const { globalToLocal } = useCoordinateConverter();

  /** 🏁 [Arrived-Checkpoint]
   * 카메라가 이미 도달하여 정착(Settle)된 지점의 식별자입니다.
   * 불필요한 재진입을 막는 '이미 도착함'의 지표입니다.
   */
  const arrivedKey = useRef<string>('');

  /** 🗺️ [Flight-Plan]
   * 'To-be Arrived' - 도달해야 할 목적지와 비행 계획입니다.
   * 실제 보간 실행은 useFrame(useCameraFrame)에서 수행.
   */
  const flyTarget = useRef<CameraFlyTarget>({
    dstPos: new THREE.Vector3(), // 카메라가 도달할 최종 물리 위치
    dstLookAt: new THREE.Vector3(), // 카메라가 바라볼 최종 지점 (Center의 Local 변환값)
  });

  /**
   * ✈️ [Flight-Planning-Engine]
   * 모든 카메라 기동(Maneuver)의 최종 목적지와 방식을 결정하는 단일 창구(Entry Point).
   */
  const planFlight = (
    dstLookAt: THREE.Vector3,
    options: { dist?: number; polar?: number; snap?: boolean } = {},
  ) => {
    const controls = controlsRef.current;
    if (!controls) return;

    const { dist, polar, snap = false } = options;

    // 🎯 [Safety Guard] Singularity(0거리 겹침) 및 NaN 방지를 위한 물리 거리 측정
    const srcDist = camera.position.distanceTo(controls.target);

    // Singularity Guard: 1m 미만 접근 시 Singularity 방지
    const finalDist = dist ?? (srcDist > MIN_DISTANCE ? srcDist : DEFAULT_DIST);
    const finalPolar =
      polar ?? (controls.getPolarAngle() || DEFAULT_POLAR_ANGLE);
    const azimuth = controls.getAzimuthalAngle();

    // [Coordinate Transformation] Spherical(Polar/Azimuth) -> Cartesian(X,Y,Z)
    const dstPosition = new THREE.Vector3(
      dstLookAt.x + finalDist * Math.sin(finalPolar) * Math.sin(azimuth),
      dstLookAt.y + finalDist * Math.cos(finalPolar),
      dstLookAt.z + finalDist * Math.sin(finalPolar) * Math.cos(azimuth),
    );

    // [Execution Strategy] 이동 거리에 따라 Snap(즉시) 또는 Fly(보간) 집행
    if (snap) {
      camera.position.copy(dstPosition);
      controls.target.copy(dstLookAt);
      controls.update(); // 렌더링 프레임 대기 없이 즉시 행렬 갱신
      setIsFlying(false);
    } else {
      flyTarget.current.dstPos.copy(dstPosition);
      flyTarget.current.dstLookAt.copy(dstLookAt);
      setIsFlying(true);
    }
  };

  /** 🔔 [Signal-Listeners] 데이터 수신 및 계획 수립부 */

  // 1. [Center Update] 외부 좌표 변화에 따른 비행 계획
  useEffect((): void => {
    const controls = controlsRef.current;
    if (!center || !controls) {
      // 🔒 [Mutex Interrupt] 사용자 조작이 개입되면 진행 중인 모든 비행 즉시 취소
      if (cameraControlMode === 'user') setIsFlying(false);
      return;
    }

    // 🎯 [Destination-Check] 'Arrived'와 'Bound'의 대조
    const boundKey = `${center.x},${center.y}`;
    if (arrivedKey.current === boundKey) return;

    /**
     * 🏁 [Coordinate-Mapping]
     * src: controls.target (현재 엔진에 그려진 물리 위치)
     * dst: center (글로벌 좌표) -> dstLookAt (엔진 로컬 좌표)
     */
    const dstLookAt = new THREE.Vector3(...toThreeCoord(globalToLocal(center)));
    const travelDist = controls.target.distanceTo(dstLookAt);

    // 비행 계획 수립 (Snap vs Fly)
    planFlight(dstLookAt, { snap: travelDist > TRAVEL_DIST_THRES });

    // 이제 이 좌표는 'Arrived'로 간주하여 중복 진입 방지
    arrivedKey.current = boundKey;
  }, [center, cameraControlMode, globalToLocal]);

  // 2. [Distance Reset] 고도(Zoom) 기본값 복구 시그널 처리
  useEffect(() => {
    const controls = controlsRef.current;
    if (!controls || resetSignal.distance === 0) return;

    // 현재 각도를 유지하며 거리만 리셋
    planFlight(controls.target, { dist: DEFAULT_DIST });
  }, [resetSignal.distance]);

  // 3. [Orientation Reset] 수직각(Polar) 기본값 복구 시그널 처리
  useEffect(() => {
    const controls = controlsRef.current;
    if (!controls || resetSignal.orientation === 0) return;

    // 현재 거리를 유지하며 각도만 리셋
    // 북쪽 정렬이 필요하다면 azimuth를 0으로 고정하겠지만,
    // 여기서는 사용자의 현재 수평각을 유지한 채 수직각(Polar)만 초기화합니다.
    planFlight(controls.target, { polar: DEFAULT_POLAR_ANGLE });
  }, [resetSignal.orientation, camera, controlsRef]);

  return { flyTarget };
}
