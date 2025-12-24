import React, { useMemo, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Line } from '@react-three/drei';

import { useMapNavigation } from '../../model/use-map-navigation';
import { VOXEL_CONFIG } from '../../model/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_LAYOUT_THEMES } from '../../../../entities/osm/config/map-theme';
import type { MeterPos } from '@/shared/types/map';

const { FOG_DENSITY, FADE_END } = THREE_CONFIG.RENDERING;

/**
 * [cell-highlighter]
 * 마우스 레이캐스팅을 통해 현재 호버 중인 격자(Voxel)를 시각화합니다.
 */
export function CellHighlighter(): ReactElement | null {
  const { raycaster, pointer, camera } = useThree();
  const {
    center,
    setCenter,
    originOffset,
    hoveredPos,
    setHoveredPos,
    themeKey,
  } = useMapNavigation();

  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const themeColor: string = targetTheme.grid.section;

  // 1. 레이캐스팅 평면 (동적으로 업데이트하기 위해 ref 사용)
  const groundPlaneRef = useRef<THREE.Plane>(
    new THREE.Plane(new THREE.Vector3(0, 1, 0), -0.1),
  );
  const intersectionPoint: THREE.Vector3 = useMemo(
    () => new THREE.Vector3(),
    [],
  );

  const localGroupRef = useRef<THREE.Group>(null);
  const aniGroupRef = useRef<THREE.Group>(null);

  // 1. 라운딩 프레임 경로 계산
  const roundedPoints: THREE.Vector3[] = useMemo((): THREE.Vector3[] => {
    const size: number = VOXEL_CONFIG.SIZE * 0.95;
    const radius: number = size * 0.15;
    const pts: THREE.Vector3[] = [];
    const steps: number = 8;
    for (let i = 0; i <= steps * 4; i++) {
      const angle: number = (i / (steps * 4)) * Math.PI * 2;
      const px: number =
        (Math.cos(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.cos(angle) * radius;
      const py: number =
        (Math.sin(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.sin(angle) * radius;
      pts.push(new THREE.Vector3(px, py, 0));
    }
    return pts;
  }, []);

  useFrame((state: { camera: THREE.Camera; clock: THREE.Clock }): void => {
    // 🎯 [fix-hover-logic] center가 없더라도 호버는 동작해야 탐색이 가능합니다.
    raycaster.setFromCamera(pointer, camera);

    /**
     * 🚀 [floating-origin-plane-sync]
     * 지도가 이동/회전하면 원점이 변합니다.
     * 평면의 높이(-0.1)를 현재 씬의 로컬 y=0 기준으로 매 프레임 재설정해야 정확한 교차점이 잡힙니다.
     */
    groundPlaneRef.current.set(new THREE.Vector3(0, 1, 0), -0.1);

    const intersect: THREE.Vector3 | null = raycaster.ray.intersectPlane(
      groundPlaneRef.current,
      intersectionPoint,
    );

    if (intersect) {
      const distToCamera: number = camera.position.distanceTo(intersect);

      // 🚀 [수정] FADE_END가 너무 짧으면 하이라이터가 일찍 사라집니다. 여유를 둡니다.
      if (distToCamera <= FADE_END * 2.0) {
        // 📍 [floating-origin-apply]
        const absolutePoint: MeterPos = {
          x: intersect.x + originOffset.x,
          y: 0,
          z: intersect.z + originOffset.z,
        };

        const snapped: MeterPos = VOXEL_CONFIG.snapToVoxel(absolutePoint);

        // 로컬 씬 좌표로 다시 변환
        const localX: number = snapped.x - originOffset.x;
        const localZ: number = snapped.z - originOffset.z;

        if (!hoveredPos || hoveredPos.x !== localX || hoveredPos.z !== localZ) {
          setHoveredPos({ x: localX, y: 0, z: localZ });
        }

        if (localGroupRef.current && hoveredPos) {
          localGroupRef.current.position.set(hoveredPos.x, 0, hoveredPos.z);

          // 🚀 2. UserMarker와 동일한 지수 안개 공식 적용
          const visibility: number = Math.exp(
            -Math.pow(distToCamera * FOG_DENSITY, 2),
          );
          localGroupRef.current.visible = visibility > 0.01;
        }
      } else {
        if (hoveredPos) setHoveredPos(null);
      }
    } else {
      if (hoveredPos) setHoveredPos(null);
    }

    // 3. 숨쉬기 애니메이션
    if (aniGroupRef.current) {
      const t: number = state.clock.getElapsedTime();
      aniGroupRef.current.position.y = 0.16 + Math.sin(t * 2.5) * 0.02;
    }
  });

  if (!hoveredPos) return null;

  return (
    <group ref={localGroupRef}>
      <group
        ref={aniGroupRef}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[VOXEL_CONFIG.toCenter(0), 0.16, VOXEL_CONFIG.toCenter(0)]}
      >
        <mesh renderOrder={99}>
          <planeGeometry
            args={[VOXEL_CONFIG.SIZE * 0.95, VOXEL_CONFIG.SIZE * 0.95]}
          />
          <meshBasicMaterial
            color={themeColor}
            transparent
            opacity={0.15}
            depthWrite={false}
          />
        </mesh>

        <mesh
          onClick={(e: React.MouseEvent): void => {
            e.stopPropagation();
            setCenter({
              x: hoveredPos.x + originOffset.x,
              y: 0,
              z: hoveredPos.z + originOffset.z,
            });
          }}
          onPointerOver={(): void => {
            document.body.style.cursor = 'pointer';
          }}
          onPointerOut={(): void => {
            document.body.style.cursor = 'auto';
          }}
        >
          <planeGeometry args={[VOXEL_CONFIG.SIZE, VOXEL_CONFIG.SIZE]} />
          <meshBasicMaterial transparent opacity={0} depthWrite={false} />
        </mesh>

        <Line
          points={roundedPoints}
          color={themeColor}
          lineWidth={3}
          transparent
          opacity={0.8}
          renderOrder={100}
        />

        {[
          [-1, -1],
          [-1, 1],
          [1, -1],
          [1, 1],
        ].map(
          ([mx, my], i): ReactElement => (
            <mesh
              key={i}
              position={[
                mx * (VOXEL_CONFIG.SIZE * 0.95 * 0.45),
                my * (VOXEL_CONFIG.SIZE * 0.95 * 0.45),
                0.01,
              ]}
              renderOrder={101}
            >
              <circleGeometry args={[0.06, 16]} />
              <meshBasicMaterial color={themeColor} />
            </mesh>
          ),
        )}
      </group>
    </group>
  );
}
