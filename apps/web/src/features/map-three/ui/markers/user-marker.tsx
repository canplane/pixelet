import { useMemo, useEffect, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import type { Vector3Tuple } from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { FloatingGroup } from '@/entities/osm/ui/floating-group';
import { useTerrainAltitude } from '@/entities/osm/lib/use-terrain-altitude';
import {
  toThreeCoord,
  toThreeCoordWithOffset,
} from '@/shared/lib/three/three-coords';

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
  geometry.rotateX(THREE_CONFIG.LAYOUT.FLOOR_ROTATION[0]);
  return geometry;
}

const ROUNDED_GEOMETRY: THREE.ShapeGeometry = createRoundedRectGeometry(
  VOXEL_CONFIG.SIZE * 0.98,
  0.8,
);
const MAX_INSTANCES = 1500;

export function UserMarker(): ReactElement | null {
  const { devicePos, accuracy, themeKey } = useMapNavigation();
  const { getTerrainAltitude } = useTerrainAltitude();

  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const tempObject = useMemo(() => new THREE.Object3D(), []);

  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const isNight = themeKey === 'night';
  const themeAccent = targetTheme.grid.section;
  const gemColor = isNight ? '#00f2ff' : '#ff1493';

  // 🎯 [Safety] 지면 높이 조회 (이제 floatingOrigin 뺄셈은 Wrapper가 함)
  const surfaceH = useMemo(() => {
    return devicePos ? getTerrainAltitude(devicePos) : 0;
  }, [devicePos, getTerrainAltitude]);

  /**
   * 🎯 [Refactoring] 복잡한 useRelativePosition 제거
   * 이제 FloatingGroup의 'coord' prop에 스냅된 절대 좌표만 넘기면 됩니다.
   * FloatingGroup 내부에서 (coord - floatingOrigin) 계산을 수행합니다.
   */
  const snappedCenter = useMemo(() => {
    if (!devicePos) return { x: 0, y: 0 };
    // 그리드 스냅 (절대 좌표계)
    const snapped = VOXEL_CONFIG.snapToVoxel(devicePos);
    return {
      x: VOXEL_CONFIG.toCenter(snapped.x),
      y: VOXEL_CONFIG.toCenter(snapped.y),
    };
  }, [devicePos]);

  // 보석(Gem)의 미세 오프셋 계산 (부드러운 이동)
  const gemOffset = useMemo(() => {
    if (!devicePos) return { x: 0, y: 0 };
    return {
      x: devicePos.x - snappedCenter.x,
      y: devicePos.y - snappedCenter.y,
    };
  }, [devicePos, snappedCenter]);

  // 🎯 [Optimization] useFrame 단순화 (위치 보정 로직 제거 -> 애니메이션만 남음)
  useFrame((state) => {
    if (!devicePos) return;

    // 1. [Terrain-Sticking] (FloatingGroup이 처리하므로 Y축 수동 갱신 불필요)
    // 단, 실시간 지형 로딩으로 surfaceH가 바뀔 수 있다면 여기서 갱신 필요할 수 있음.
    // 현재는 useMemo(surfaceH)가 devicePos 변경 시 갱신됨.

    // 2. [Visibility & Animation]
    if (gemRef.current && meshRef.current) {
      const markerWorldPos = new THREE.Vector3();
      gemRef.current.getWorldPosition(markerWorldPos);
      const distance = state.camera.position.distanceTo(markerWorldPos);

      const isVisible = distance < THREE_CONFIG.RENDERING.FOG_FAR;

      gemRef.current.visible = isVisible;
      meshRef.current.visible = isVisible;

      if (isVisible) {
        const t = state.clock.getElapsedTime();
        gemRef.current.rotation.y = t * 1.2;
        // 로컬 좌표계에서의 상하 운동
        gemRef.current.position.y = 2 + Math.sin(t * 1.5) * 0.25;
      }
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
        if ((x * step) ** 2 + (y * step) ** 2 <= activeAccuracy ** 2) {
          offsets.push({ x: x * step, y: y * step });
        }
      }
    }
    return offsets.slice(0, MAX_INSTANCES);
  }, [accuracy, devicePos]);

  useEffect((): void => {
    if (!meshRef.current) return;

    const hiddenPos = toThreeCoord({ x: 0, y: 0, z: -1000 });

    for (let i = 0; i < MAX_INSTANCES; i++) {
      tempObject.position.set(...hiddenPos);
      tempObject.updateMatrix();
      meshRef.current.setMatrixAt(i, tempObject.matrix);
    }

    gridOffsets.forEach((offset, i) => {
      const pos: Vector3Tuple = toThreeCoord({
        x: offset.x,
        y: offset.y,
        z: 0.05,
      });

      tempObject.position.set(...pos);
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(i, tempObject.matrix);
    });

    meshRef.current.instanceMatrix.needsUpdate = true;
    meshRef.current.count = gridOffsets.length;
  }, [gridOffsets, tempObject]);

  if (!devicePos) return null;

  // 🎯 [Height Correction]
  // FloatingGroup에 넘겨줄 높이 값 (지형 높이 + 시스템 오프셋)
  // FloatingGroup 내부에서 (absoluteHeight - floatingOrigin.z) 처리를 수행합니다.
  const absoluteHeight = surfaceH + VOXEL_CONFIG.SYSTEM_OFFSET;

  // Gem의 위치는 그룹 내부의 로컬 좌표이므로 toThreeCoordWithOffset 그대로 사용
  const gemPos = toThreeCoordWithOffset({ x: gemOffset.x, y: gemOffset.y }, 0);
  const lightPos = toThreeCoord({ x: gemOffset.x, y: gemOffset.y });

  return (
    // 🎯 [Wrapper Pattern] 위치 잡는 책임을 FloatingGroup에 위임
    <FloatingGroup coord={{ ...snappedCenter, z: absoluteHeight }}>
      <instancedMesh
        ref={meshRef}
        args={[ROUNDED_GEOMETRY, undefined, MAX_INSTANCES]}
        renderOrder={5}
      >
        <meshBasicMaterial
          color={isNight ? themeAccent : '#FFD700'}
          transparent
          opacity={0.5}
          depthWrite={false}
        />
      </instancedMesh>
      <mesh ref={gemRef} position={gemPos}>
        <octahedronGeometry args={[0.6, 0]} />
        <meshStandardMaterial
          color={gemColor}
          emissive={gemColor}
          emissiveIntensity={isNight ? 2 : 0.5}
          roughness={0.2}
          metalness={0.8}
          flatShading
        />
      </mesh>
      <pointLight
        position={lightPos}
        distance={16}
        intensity={isNight ? 10 : 3}
        color={gemColor}
      />
    </FloatingGroup>
  );
}
