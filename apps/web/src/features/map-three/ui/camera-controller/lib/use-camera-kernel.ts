import { useRef } from 'react';
import * as THREE from 'three';
import type { Vector3Tuple } from 'three';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';

import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { useCoordinateConverter } from '@/entities/osm/lib/use-coordinate-converter';
import { fromThreeCoord } from '@/shared/lib/three/three-coords';

import { useCameraInitialization } from './use-camera-initialization';
import { useCameraMovement } from './use-camera-movement';
import { useCameraFrame } from './use-camera-frame';
import { useCameraTerrainAlign } from '@/features/map-three/ui/camera-controller/lib/use-camera-terrain-align';
import { useCameraFloatingOrigin } from '@/features/map-three/ui/camera-controller/lib/use-camera-floating-origin';

/**
 * 🧠 [useCameraKernel]
 * 분산된 카메라 기동/보정 훅들을 하나로 통합하여 내부 상태(flyState 등)를 캡슐화함.
 */
export function useCameraKernel() {
  const controlsRef = useRef<OrbitControlsImpl>(null);

  const { updateCenter, setIsFlying, setIsFollowMode } = useMapNavigation();
  const { localToGlobal } = useCoordinateConverter();

  // 초기화 로직
  useCameraInitialization();

  // ✈️ 비행 상태를 커널이 소유
  // 이동 및 동기화, 시그널 처리 로직 (flyState 반환)
  const { flyTarget } = useCameraMovement({ controlsRef });

  // Floating Origin 처리 로직
  useCameraFloatingOrigin({ controlsRef });

  // 지형 밀착
  useCameraTerrainAlign({ controlsRef, flyTarget });

  // 프레임 루프 로직 (애니메이션)
  useCameraFrame({ controlsRef, flyTarget });

  /** 핸들러 분리: 커널 제어권 전환 로직 */
  const onInteractionStart = () => {
    // 부수 효과 처리
    setIsFollowMode(false);

    // 실행 중인 비행 강제 종료 (이미 useCameraMovement 내부에 있지만 더블 가드)
    setIsFlying(false);
  };

  const onInteractionEnd = () => {
    const controls = controlsRef.current;
    if (!controls) return;

    const { target } = controls;
    const targetVec: Vector3Tuple = [target.x, target.y, target.z];

    // 수치 유효성 검증
    if (!targetVec.every(Number.isFinite)) return;

    // Rebase: 물리적 위치를 전역 도메인으로 변환하여 보고
    // 이 과정에서 Floating Origin의 originShift가 발생할 수 있음
    const globalPos = localToGlobal(fromThreeCoord(targetVec));
    updateCenter(globalPos);
  };

  return { controlsRef, onInteractionStart, onInteractionEnd };
}
