import { useMemo, useRef, useEffect, useState, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';

import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { useTerrainAltitude } from '@/entities/osm/lib/use-terrain-altitude';
import { fromThreeCoord } from '@/shared/lib/three/three-coords';
import { FloatingGroup } from '@/entities/osm/ui/floating-group';
import type { Coord } from '@pixelet/shared/shared/model/coords';

// 🎯 [Type Definition] 하이라이터가 표시할 정보
interface HoveredInfo {
  coord: Coord; // 절대 좌표
}

export function CellHighlighter(): ReactElement | null {
  const { gl, raycaster, controls } = useThree();
  const { updateCenter, floatingOrigin, themeKey } = useMapNavigation();
  const { getTerrainAltitude } = useTerrainAltitude();

  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const themeColor = targetTheme.grid.section;

  // 1. 계산용 임시 객체들 (메모리 할당 최소화)
  const intersectionPoint = useMemo(() => new THREE.Vector3(), []);
  const plane = useMemo(
    () => new THREE.Plane(new THREE.Vector3(0, 1, 0), 0),
    [],
  );

  // 2. 상태 관리 (격자가 바뀔 때만 렌더링 발생)
  const [hoveredInfo, setHoveredInfo] = useState<HoveredInfo | null>(null);

  // 3. 클릭 이벤트를 위한 Ref (리렌더링 없이 최신 좌표 추적)
  const latestHoveredRef = useRef<HoveredInfo | null>(null);

  // 포인터 트래킹
  const pointerDownTime = useRef(0);
  const pointerDownPos = useRef(new THREE.Vector2());

  // 🚀 [Logic] Raycasting Loop
  useFrame(() => {
    if (!controls) return;

    // (1) 가상의 바닥 평면을 현재 카메라 타겟 높이에 맞춤
    const currentTargetY = (controls as any).target.y;
    plane.constant = -currentTargetY;

    // (2) Raycasting
    const hit = raycaster.ray.intersectPlane(plane, intersectionPoint);

    if (hit) {
      // (3) Local(Three.js) -> Global(Absolute) 변환
      const globalPos = fromThreeCoord([hit.x, hit.y, hit.z]);

      // Floating Origin 보정 (현재 렌더링 기준 -> 실제 절대 좌표)
      const absoluteX = globalPos.x + floatingOrigin.x;
      const absoluteY = globalPos.y + floatingOrigin.y;

      // (4) 격자 스냅 (Voxel Grid)
      const snapped = VOXEL_CONFIG.snapToVoxel({
        x: absoluteX,
        y: absoluteY,
        z: 0,
      });
      const gx = VOXEL_CONFIG.toCenter(snapped.x);
      const gy = VOXEL_CONFIG.toCenter(snapped.y);

      // (5) 지형 높이 조회
      const terrainH = getTerrainAltitude({ x: gx, y: gy }) ?? 0;
      const targetHeight = terrainH + 0.15; // Z-fighting 방지 오프셋

      // (6) 상태 업데이트 (좌표가 변경되었을 때만!)
      const current = latestHoveredRef.current;
      if (!current || current.coord.x !== gx || current.coord.y !== gy) {
        const newInfo = { coord: { x: gx, y: gy, z: targetHeight } };

        latestHoveredRef.current = newInfo; // 클릭용 Ref 업데이트
        setHoveredInfo(newInfo); // 렌더링용 State 업데이트
      }
    } else {
      // Ray가 빗나가면 숨김
      if (latestHoveredRef.current !== null) {
        latestHoveredRef.current = null;
        setHoveredInfo(null);
      }
    }
  });

  // 🚀 [Interaction] 클릭 핸들러 (DOM Event)
  useEffect(() => {
    const canvas = gl.domElement;

    const handleDown = (e: PointerEvent) => {
      pointerDownTime.current = Date.now();
      pointerDownPos.current.set(e.clientX, e.clientY);
    };

    const handleUp = (e: PointerEvent) => {
      const duration = Date.now() - pointerDownTime.current;
      const moveDist = pointerDownPos.current.distanceTo(
        new THREE.Vector2(e.clientX, e.clientY),
      );

      // 짧은 클릭(Tap)이고, 하이라이트 된 곳이 있다면 이동
      if (duration < 250 && moveDist < 10 && latestHoveredRef.current) {
        const { coord } = latestHoveredRef.current;

        // MapCameraController 감지용 미세 오차 주입
        const target = {
          x: coord.x,
          y: coord.y,
          z: coord.z + Math.random() * 0.0001,
        };

        console.log('🚀 Move to:', target);
        updateCenter(target);
      }
    };

    canvas.addEventListener('pointerdown', handleDown);
    canvas.addEventListener('pointerup', handleUp);
    return () => {
      canvas.removeEventListener('pointerdown', handleDown);
      canvas.removeEventListener('pointerup', handleUp);
    };
  }, [gl, updateCenter]);

  // 데이터가 없으면 렌더링 안 함
  if (!hoveredInfo) return null;

  return (
    // 🎯 [Pattern] 절대 좌표만 넘기면 FloatingGroup이 알아서 위치를 잡습니다.
    <FloatingGroup coord={hoveredInfo.coord}>
      <group rotation={[-Math.PI / 2, 0, 0]}>
        <mesh renderOrder={9999999}>
          <planeGeometry args={[VOXEL_CONFIG.SIZE, VOXEL_CONFIG.SIZE]} />
          <meshBasicMaterial
            color={themeColor}
            transparent
            opacity={0.6}
            depthTest={false}
            depthWrite={false}
            blending={THREE.AdditiveBlending}
          />
        </mesh>
      </group>
    </FloatingGroup>
  );
}
