import React, { useMemo, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Line } from '@react-three/drei';

import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

const { FOG_DENSITY, FADE_END } = THREE_CONFIG.RENDERING;

/**
 * [cell-highlighter]
 * 마우스 레이캐스팅을 통해 현재 호버 중인 격자(Voxel)를 시각화합니다.
 */
export function CellHighlighter(): ReactElement | null {
  const { raycaster, pointer, camera } = useThree();
  const { setCenter, origin, hoveredPos, setHoveredPos, themeKey } =
    useMapNavigation();

  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const themeColor: string = targetTheme.grid.section;

  // 🎯 평면은 월드 Y=0(바닥)에 고정
  const groundPlane = useMemo(
    () => new THREE.Plane(new THREE.Vector3(0, 1, 0), 0),
    [],
  );
  const intersectionPoint = useMemo(() => new THREE.Vector3(), []);

  const localGroupRef = useRef<THREE.Group>(null);
  const aniGroupRef = useRef<THREE.Group>(null);

  // 1. 라운딩 프레임 경로 계산
  const roundedPoints = useMemo(() => {
    const size = VOXEL_CONFIG.SIZE * 0.95;
    const radius = size * 0.15;
    const pts = [];
    for (let i = 0; i <= 32; i++) {
      const angle = (i / 32) * Math.PI * 2;
      const px =
        (Math.cos(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.cos(angle) * radius;
      const py =
        (Math.sin(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.sin(angle) * radius;
      pts.push(new THREE.Vector3(px, py, 0));
    }
    return pts;
  }, []);

  useFrame((state: { camera: THREE.Camera; clock: THREE.Clock }): void => {
    raycaster.setFromCamera(pointer, camera);
    const intersect = raycaster.ray.intersectPlane(
      groundPlane,
      intersectionPoint,
    );

    if (intersect) {
      const distToCamera: number = camera.position.distanceTo(intersect);

      // 🚀 [수정] FADE_END가 너무 짧으면 하이라이터가 일찍 사라집니다. 여유를 둡니다.
      if (distToCamera <= FADE_END * 2.0) {
        /**
         * 🎯 [Coordinate-Fix]
         * 1. 씬의 로컬 X, Z를 지리적 절대 좌표로 복원
         * - Three.js X -> 지리적 X
         * - Three.js -Z -> 지리적 Y (북쪽)
         */
        const absolutePoint: MeterPos = {
          x: intersect.x + origin.x,
          y: -intersect.z + origin.y, // 🎯 축 스와핑 핵심
          z: 0,
        };

        // 2. 스냅 (4m 단위 격자에 맞춤)
        const snapped = VOXEL_CONFIG.snapToVoxel(absolutePoint);

        // 3. 다시 씬의 로컬 좌표로 변환
        const localX = snapped.x - origin.x;
        const localY = snapped.y - origin.y;

        // 🎯 [Center-Correction]
        // 하이라이터 메시는 '중심' 기준이므로, 스냅된 좌표의 '가운데'로 보정합니다.
        const centeredX = VOXEL_CONFIG.toCenter(localX);
        const centeredY = VOXEL_CONFIG.toCenter(localY);

        if (
          !hoveredPos ||
          hoveredPos.x !== centeredX ||
          hoveredPos.y !== centeredY
        ) {
          setHoveredPos({ x: centeredX, y: centeredY, z: 0 });
        }

        if (localGroupRef.current) {
          // 🎯 Three.js 위치 배치: (X, 0, -Y)
          localGroupRef.current.position.set(centeredX, 0, -centeredY);

          const visibility = Math.exp(-Math.pow(distToCamera * FOG_DENSITY, 2));
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
      const t = state.clock.getElapsedTime();
      aniGroupRef.current.position.y = 0.05 + Math.sin(t * 2.5) * 0.02;
    }
  });

  if (!hoveredPos) return null;

  return (
    <group ref={localGroupRef}>
      <group
        ref={aniGroupRef}
        rotation={[-Math.PI / 2, 0, 0]} // 바닥에 눕힘
        // 🚀 [Position-Reset] 부모 그룹이 이미 중심 좌표를 잡았으므로 내부는 0,0,0
        position={[0, 0, 0]}
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

        {/* 클릭 영역 (투명) */}
        <mesh
          onClick={(e) => {
            e.stopPropagation();
            // 🎯 setCenter 시에도 절대 좌표(origin 포함)로 전달
            setCenter({
              x: hoveredPos.x + origin.x,
              y: hoveredPos.y + origin.y,
              z: 0,
            });
          }}
          onPointerOver={() => (document.body.style.cursor = 'pointer')}
          onPointerOut={() => (document.body.style.cursor = 'auto')}
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

        {/* 네 모서리 도트 */}
        {[
          [-1, -1],
          [-1, 1],
          [1, -1],
          [1, 1],
        ].map(([mx, my], i) => (
          <mesh
            key={i}
            position={[
              mx * (VOXEL_CONFIG.SIZE * 0.42),
              my * (VOXEL_CONFIG.SIZE * 0.42),
              0.01,
            ]}
          >
            <circleGeometry args={[0.06, 16]} />
            <meshBasicMaterial color={themeColor} />
          </mesh>
        ))}
      </group>
    </group>
  );
}
