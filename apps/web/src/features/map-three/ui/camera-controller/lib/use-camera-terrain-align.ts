import { useLayoutEffect, type RefObject } from 'react';
import { useThree } from '@react-three/fiber';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import { useTerrainAltitude } from '@/entities/geo/lib/use-terrain-altitude';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import type { CameraFlyTarget } from './use-camera-movement';

interface UseCameraTerrainAlignProps {
  controlsRef: React.RefObject<OrbitControlsImpl | null>;
  flyTarget: RefObject<CameraFlyTarget>;
}

/**
 * 🎯 [Center-Reactive-Align]
 * useFrame 없이 center가 변경될 때만 해당 좌표의 지형 높이를 조회하여
 * 카메라와 타겟의 고도(Y)를 보정합니다.
 */
export function useCameraTerrainAlign({
  controlsRef,
  flyTarget,
}: UseCameraTerrainAlignProps) {
  const { center, floatingOrigin, isFlying } = useMapNavigation();
  const { camera } = useThree();
  const { getTerrainAltitude } = useTerrainAltitude();

  useLayoutEffect(() => {
    const controls = controlsRef.current;
    if (!controls || !center) return;

    // 1. 현재 전역 좌표(center)의 실제 지형 높이 조회
    const gh = getTerrainAltitude(center);

    // 2. Three.js 로컬 Y 좌표 계산 (Anchor Z값 보정)
    const targetThreeY = (gh ?? 0) - floatingOrigin.z;

    // 3. 현재 타겟 높이와 지형 높이 비교
    const currentTy = controls.target.y;

    // 델타값 계산 (지형 변화량)
    const deltaY = targetThreeY - currentTy;

    if (Math.abs(deltaY) < 0.01) return;

    if (isFlying) {
      /**
       * ✈️ [Flight-Correction]
       * 비행 중이라면 현재 물리 위치를 건드리지 않고,
       * 도착지(dst)의 높이만 지형에 맞춰 수정합니다.
       * 이렇게 하면 보간이 자연스럽게 지형 위로 착륙합니다.
       */
      flyTarget.current.dstLookAt.y = targetThreeY;
      flyTarget.current.dstPos.y += deltaY;
    } else {
      /**
       * ⚓ [Static-Correction]
       * 정지 상태(또는 Snap)일 때는 기존처럼 즉시 이동
       */
      camera.position.y += deltaY;
      controls.target.y = targetThreeY;

      // 행렬 갱신
      controls.update();
    }
  }, [center, floatingOrigin.z, getTerrainAltitude, camera, flyTarget]);
}
