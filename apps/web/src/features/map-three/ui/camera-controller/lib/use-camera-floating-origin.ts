import { useLayoutEffect, useRef } from 'react';
import * as THREE from 'three';
import { useThree } from '@react-three/fiber';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import type { Coord } from '@pixelet/core/shared/model/coords';
import { toThreeCoord } from '@/shared/lib/three/three-coords';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import {
  getTileCoord,
  getTileKey,
} from '@pixelet/core/entities/osm/lib/utils/tile-utils';

interface UseCameraFloatingOriginProps {
  controlsRef: React.RefObject<OrbitControlsImpl | null>;
}

/**
 * 🎯 [Floating-Origin-Sync]
 * useFrame을 제거하고 center 변화에 따라 원점 리베이스를 수행합니다.
 * useLayoutEffect를 사용하여 좌표 이동 시 발생하는 미세한 깜빡임을 차단합니다.
 */
export function useCameraFloatingOrigin({
  controlsRef,
}: UseCameraFloatingOriginProps) {
  const { floatingOrigin, isFlying } = useMapNavigation();
  const { camera } = useThree();

  // 이전 floating origin을 추적하기 위한 ref
  const lastFloatingOriginRef = useRef<Coord>(floatingOrigin);

  useLayoutEffect(() => {
    const controls = controlsRef.current;

    // 1. 가드 로직: floating origin이 없거나 조작 주체가 다르거나 물리 객체가 없으면 스킵
    if (!floatingOrigin || !controls || isFlying) {
      lastFloatingOriginRef.current = floatingOrigin;
      return;
    }

    // 2. 변경 감지 및 시프팅 (Single-pass logic)
    const dx = floatingOrigin.x - lastFloatingOriginRef.current.x;
    const dy = floatingOrigin.y - lastFloatingOriginRef.current.y;

    if (dx !== 0 || dy !== 0) {
      const moveVec = new THREE.Vector3(...toThreeCoord({ x: dx, y: dy }));

      // 🎯 [Atomic Shift] 모든 물리 객체를 한 번에 밀어냄
      camera.position.sub(moveVec);
      controls.target.sub(moveVec);

      // 🔄 강제 동기화
      camera.updateMatrixWorld(true);
      controls.update();

      lastFloatingOriginRef.current = floatingOrigin;

      console.log(
        `[Floating Origin] Shifted to Tile (${getTileKey(getTileCoord(floatingOrigin))})`,
      );
    }
  }, [floatingOrigin, camera, controlsRef, isFlying]);
}
