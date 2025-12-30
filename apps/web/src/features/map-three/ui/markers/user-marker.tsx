import { useMemo, useEffect, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { useRelativePosition } from '../../../../entities/osm/lib/use-coordinate-system';
import {
  toThreePos,
  toThreePosWithOffset,
} from '@/shared/lib/three/three-coords';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { useMapStore } from '@/entities/osm/model/map-store';

const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO } = OSM_CONSTANTS;

/**
 * [create-rounded-rect-geometry]
 * 마커 하단의 정확도 범위를 표시하는 둥근 사각형 지오메트리를 생성합니다.
 */
function createRoundedRectGeometry(
  size: number,
  radius: number,
): THREE.ShapeGeometry {
  const shape: THREE.Shape = new THREE.Shape();
  const h: number = size / 2;

  shape.moveTo(-h, -h + radius);
  shape.lineTo(-h, h - radius);
  shape.quadraticCurveTo(-h, h, -h + radius, h);
  shape.lineTo(h - radius, h);
  shape.quadraticCurveTo(h, h, h, h - radius);
  shape.lineTo(h, -h + radius);
  shape.quadraticCurveTo(h, -h, h - radius, -h);
  shape.lineTo(-h + radius, -h);
  shape.quadraticCurveTo(-h, -h, -h, -h + radius);

  const geometry: THREE.ShapeGeometry = new THREE.ShapeGeometry(shape);
  // [geometry-initial-rotation] 바닥에 눕혀 좌표계 일관성을 유지합니다.
  geometry.rotateX(THREE_CONFIG.LAYOUT.FLOOR_ROTATION[0]);
  return geometry;
}

const ROUNDED_GEOMETRY: THREE.ShapeGeometry = createRoundedRectGeometry(
  VOXEL_CONFIG.SIZE * 0.98,
  0.8,
);
const MAX_INSTANCES: number = 1500;

/**
 * [user-marker]
 * 사용자의 현재 위치와 GPS 정확도 범위를 시각화하는 마커입니다.
 */
export function UserMarker(): ReactElement | null {
  const { devicePos, accuracy, themeKey } = useMapNavigation();
  const tiles = useMapStore((state) => state.tiles); // 🎯 전역 타일 데이터

  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null); // 🎯 고도 적용을 위해 그룹 참조 추가

  const tempObject: THREE.Object3D = useMemo(() => new THREE.Object3D(), []);

  // [hook-order-safety]
  // 에러 방지를 위해 모든 Hook은 조기 리턴(if !devicePos)보다 상단에 위치해야 합니다.
  // 실제 값 유무와 상관없이 Hook은 동일한 순서로 실행되어야 합니다.
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const isNight: boolean = themeKey === 'night';
  const themeAccent: string = targetTheme.grid.section;
  const gemColor: string = isNight ? '#00f2ff' : '#ff1493';

  // 🎯 [Coordinate-Fix] 평면 위치 계산을 위해 고도(z)를 0으로 고정
  const basePos = useMemo(
    () =>
      devicePos
        ? { x: devicePos.x, y: devicePos.y, z: 0 }
        : { x: 0, y: 0, z: 0 },
    [devicePos],
  );

  const localPos = useRelativePosition(basePos);

  const snappedLocalPos = useRelativePosition(
    useMemo(
      () =>
        devicePos ? VOXEL_CONFIG.snapToVoxel(devicePos) : { x: 0, y: 0, z: 0 },
      [devicePos],
    ),
  );

  // 🎯 [Offset-Fix] 동서(x)와 남북(y) 오프셋을 정확히 분리
  const gemOffset = useMemo(() => {
    return {
      x: localPos.x - VOXEL_CONFIG.toCenter(snappedLocalPos.x),
      y: localPos.y - VOXEL_CONFIG.toCenter(snappedLocalPos.y), // 지리적 Y (남북)
    };
  }, [localPos, snappedLocalPos]);

  /**
   * 🏔️ [Ground Clamping Logic]
   * 현재 기기 위치(devicePos)가 속한 타일을 찾아 해당 좌표의 고도값을 반환합니다.
   */
  const getElevation = (x: number, y: number): number => {
    // 1. 현재 좌표가 속한 타일 키 계산 (tx, ty)
    const tx = Math.floor(x / TILE_SIZE);
    const ty = Math.floor(y / TILE_SIZE);
    const tileKey = `${tx}:${ty}`;

    const tile = tiles.get(tileKey);
    if (!tile || !tile.grids.terrain.height) return 0;

    // 2. 타일 내 로컬 좌표로 변환 (0 ~ TILE_SIZE)
    const localX = x - tx * TILE_SIZE;
    const localY = y - ty * TILE_SIZE;

    // 3. 그리드 인덱스 계산 (Resolution: 2048)
    const res = Math.floor(TILE_SIZE / CELL_SIZE);
    const gridX = Math.floor(localX / CELL_SIZE);
    const gridY = Math.floor((TILE_SIZE - localY) / CELL_SIZE); // Top-Down 보정

    const index = gridY * res + gridX;
    const hRaw = tile.grids.terrain.height[index] || 0;

    // 4. 물리적 고도 계산 (Uint16 * Z_Unit)
    return hRaw * (CELL_SIZE * Z_SCALE_RATIO);
  };

  useFrame((state: { camera: THREE.Camera; clock: THREE.Clock }): void => {
    if (!devicePos || !groupRef.current) return;

    // 🎯 [Height-Sync] 매 프레임 현재 위치의 고도를 계산하여 그룹의 Y(Three.js Z) 축에 적용
    const elevation = getElevation(devicePos.x, devicePos.y);

    // Three.js 좌표계: 지면 고도는 Y축 (우리 레이아웃 기준으론 Z가 고도일 수 있으나
    // 현재 그룹의 position 배열 구조에 맞춰 조정)
    groupRef.current.position.y = elevation + VOXEL_CONFIG.SYSTEM_OFFSET;

    if (!gemRef.current || !meshRef.current || !devicePos) return;

    const markerWorldPos: THREE.Vector3 = new THREE.Vector3();
    gemRef.current.getWorldPosition(markerWorldPos);
    const distance: number = state.camera.position.distanceTo(markerWorldPos);
    const fadeEnd: number = THREE_CONFIG.RENDERING.FADE_END;

    const visibility: number = Math.exp(
      -Math.pow(distance * THREE_CONFIG.RENDERING.FOG_DENSITY, 2),
    );
    const isVisible: boolean = visibility > 0.01 && distance < fadeEnd * 1.5;

    gemRef.current.visible = isVisible;
    meshRef.current.visible = isVisible;

    if (isVisible) {
      const t: number = state.clock.getElapsedTime();
      gemRef.current.rotation.y = t * 1.2;
      gemRef.current.position.y = 2 + Math.sin(t * 1.5) * 0.25;
    }
  });

  const gridOffsets = useMemo((): { x: number; y: number }[] => {
    if (!devicePos) return [];
    const activeAccuracy: number = Math.min(accuracy || 10, 50);
    const offsets: { x: number; y: number }[] = [];
    const step: number = VOXEL_CONFIG.SIZE;
    const range: number = Math.ceil(activeAccuracy / step);

    for (let x = -range; x <= range; x++) {
      for (let y = -range; y <= range; y++) {
        // 평면 원형 범위 체크 (x:동서, y:남북)
        if ((x * step) ** 2 + (y * step) ** 2 <= activeAccuracy ** 2) {
          offsets.push({ x: x * step, y: y * step });
        }
      }
    }
    return offsets.slice(0, MAX_INSTANCES);
  }, [accuracy, devicePos]);

  useEffect((): void => {
    if (!meshRef.current) return;

    // 초기화: 모든 인스턴스를 보이지 않는 곳으로 이동
    for (let i = 0; i < MAX_INSTANCES; i++) {
      tempObject.position.set(0, -1000, 0);
      tempObject.updateMatrix();
      meshRef.current.setMatrixAt(i, tempObject.matrix);
    }

    // 활성 격자 배치
    gridOffsets.forEach((offset, i) => {
      /**
       * 🎯 [Swizzling-Fix]
       * InstancedMesh 내부 좌표 배치:
       * Three X = offset.x (동서)
       * Three Y = 0.05 (지면 높이)
       * Three Z = -offset.y (남북 -> 북향 매핑)
       */
      tempObject.position.set(offset.x, 0.05, -offset.y);
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(i, tempObject.matrix);
    });

    meshRef.current.instanceMatrix.needsUpdate = true;
    meshRef.current.count = gridOffsets.length;
  }, [gridOffsets, tempObject]);

  // [type-guard-render] 모든 Hook 실행 후 렌더링 여부 결정
  if (!devicePos) return null;

  return (
    <group
      ref={groupRef} // 🎯 고도 제어를 위해 ref 연결
      position={[
        VOXEL_CONFIG.toCenter(snappedLocalPos.x),
        VOXEL_CONFIG.SYSTEM_OFFSET,
        -VOXEL_CONFIG.toCenter(snappedLocalPos.y), // y -> -z (북향 매핑)
      ]}
    >
      {/* 🎯 InstancedMesh (정확도 범위) */}
      <instancedMesh
        ref={meshRef}
        args={[ROUNDED_GEOMETRY, undefined, MAX_INSTANCES]}
        renderOrder={5} //
      >
        <meshBasicMaterial
          color={isNight ? themeAccent : '#FFD700'}
          transparent
          opacity={0.5}
          depthWrite={false}
        />
      </instancedMesh>

      {/* 🎯 Gem & Light: 사용자 위치를 표시하는 보석 오브젝트 */}
      <mesh ref={gemRef} position={toThreePosWithOffset(gemOffset, 0)}>
        <octahedronGeometry args={[0.6, 0]} />
        <meshStandardMaterial
          color={gemColor}
          emissive={gemColor}
          emissiveIntensity={isNight ? 2 : 0.5}
          roughness={0.2} //
          metalness={0.8} ///
          flatShading
        />
      </mesh>

      {/* [location-glow] 전역 디자인 가이드에 따른 야간 발광 효과 */}
      <pointLight
        position={toThreePos(gemOffset)}
        distance={16}
        intensity={isNight ? 10 : 3}
        color={gemColor}
      />
    </group>
  );
}
