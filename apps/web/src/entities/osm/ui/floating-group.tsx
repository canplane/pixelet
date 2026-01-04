import { useRef, useLayoutEffect, type ReactNode } from 'react';
import * as THREE from 'three';
import { useMapStore } from '../model/map-store'; // 같은 Entity 내 Import
import { toThreeCoord } from '@/shared/lib/three/three-coords';
import type { Coord } from '@pixelet/core/shared/model/coords';

interface FloatingGroupProps {
  /** 객체의 절대 좌표 (Global Coordinate) */
  coord: Coord;
  children: ReactNode;
}

/**
 * [FloatingGroup]
 * Floating Origin 시스템 하에서 객체의 절대 좌표를 받아
 * 현재 원점(Floating Origin) 기준의 로컬 좌표로 자동 변환/배치해주는 래퍼입니다.
 */
export function FloatingGroup({ coord, children }: FloatingGroupProps) {
  const groupRef = useRef<THREE.Group>(null);

  // Entity의 Store를 직접 구독하여 사용성을 높입니다.
  const floatingOrigin = useMapStore((state) => state.floatingOrigin);

  useLayoutEffect(() => {
    if (!groupRef.current) return;

    // 1. 상대 좌표 계산 (절대 좌표 - 현재 원점)
    const dx = coord.x - floatingOrigin.x;
    const dy = coord.y - floatingOrigin.y;

    // 2. Three.js 좌표계 변환 (Floating Origin의 z는 보통 0이거나 지형 높이 고려)
    // * 만약 floatingOrigin.z까지 고려한다면 (height - floatingOrigin.z)가 됩니다.
    const z = coord.z - floatingOrigin.z;

    const localPos = toThreeCoord({ x: dx, y: dy });

    // 3. 위치 설정
    groupRef.current.position.set(localPos[0], z, localPos[2]);
    groupRef.current.updateMatrix(); // AutoUpdate가 꺼져있을 경우를 대비해 안전하게 호출
  }, [floatingOrigin, coord]);

  return <group ref={groupRef}>{children}</group>;
}
