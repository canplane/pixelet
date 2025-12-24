import { useMemo, useEffect, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import type { MeterPos } from '@/shared/types/map';
import { useMapNavigation } from '../../model/use-map-navigation';
import { VOXEL_CONFIG } from '../../model/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_LAYOUT_THEMES } from '../../../../entities/osm/config/map-theme';
import { useRelativePosition } from '../../model/use-relative-position';

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
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const tempObject: THREE.Object3D = useMemo(() => new THREE.Object3D(), []);

  // [hook-order-safety]
  // 에러 방지를 위해 모든 Hook은 조기 리턴(if !devicePos)보다 상단에 위치해야 합니다.
  // 실제 값 유무와 상관없이 Hook은 동일한 순서로 실행되어야 합니다.
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];
  const isNight: boolean = themeKey === 'night';
  const themeAccent: string = targetTheme.grid.section;
  const gemColor: string = isNight ? '#00f2ff' : '#ff1493';

  // [relative-position-sync]
  // devicePos가 없을 경우 기본값({x:0, y:0, z:0})을 활용하여 훅 실행 순서를 유지합니다.
  const localPos: MeterPos = useRelativePosition(
    useMemo(
      () => (devicePos ? { ...devicePos, y: 0 } : { x: 0, y: 0, z: 0 }),
      [devicePos],
    ),
  );

  const snappedLocalPos: MeterPos = useRelativePosition(
    useMemo(
      () =>
        devicePos ? VOXEL_CONFIG.snapToVoxel(devicePos) : { x: 0, y: 0, z: 0 },
      [devicePos],
    ),
  );

  const gemOffset = useMemo((): { x: number; z: number } => {
    return {
      x: localPos.x - VOXEL_CONFIG.toCenter(snappedLocalPos.x),
      z: localPos.z - VOXEL_CONFIG.toCenter(snappedLocalPos.z),
    };
  }, [localPos, snappedLocalPos]);

  useFrame((state: { camera: THREE.Camera; clock: THREE.Clock }): void => {
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

  const gridOffsets = useMemo((): { x: number; z: number }[] => {
    if (!devicePos) return [];
    const activeAccuracy: number = Math.min(accuracy || 10, 50);
    const offsets: { x: number; z: number }[] = [];
    const step: number = VOXEL_CONFIG.SIZE;
    const range: number = Math.ceil(activeAccuracy / step);

    for (let x = -range; x <= range; x++) {
      for (let z = -range; z <= range; z++) {
        if ((x * step) ** 2 + (z * step) ** 2 <= activeAccuracy ** 2) {
          offsets.push({ x: x * step, z: z * step });
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
    gridOffsets.forEach((offset: { x: number; z: number }, i: number): void => {
      tempObject.position.set(offset.x, 0.125, offset.z);
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
      position={[
        VOXEL_CONFIG.toCenter(snappedLocalPos.x),
        VOXEL_CONFIG.SYSTEM_OFFSET,
        VOXEL_CONFIG.toCenter(snappedLocalPos.z),
      ]}
    >
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

      {/* [floating-gem] 사용자 위치를 표시하는 보석 오브젝트 */}
      <mesh
        ref={gemRef}
        position={[gemOffset.x, 2, gemOffset.z]}
        renderOrder={20}
      >
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

      {/* [location-glow] 전역 디자인 가이드에 따른 야간 발광 효과 */}
      <pointLight
        position={[gemOffset.x, 2, gemOffset.z]}
        distance={16}
        intensity={isNight ? 10 : 3}
        color={gemColor}
      />
    </group>
  );
}
