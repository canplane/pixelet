import { useMemo, useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapNavigation } from '../../model/use-map-navigation';
import { VOXEL_CONFIG } from '../../model/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_THEMES } from '../../model/map-theme';
const { FOG_DENSITY } = THREE_CONFIG.RENDERING;

function createRoundedRectGeometry(size: number, radius: number) {
  const shape = new THREE.Shape();

  // 🚀 -size/2 로 시작하여 메쉬의 (0,0,0)이 정확히 사각형의 '중심'이 되게 합니다.
  const h = size / 2;

  shape.moveTo(-h, -h + radius);
  shape.lineTo(-h, h - radius);
  shape.quadraticCurveTo(-h, h, -h + radius, h);
  shape.lineTo(h - radius, h);
  shape.quadraticCurveTo(h, h, h, h - radius);
  shape.lineTo(h, -h + radius);
  shape.quadraticCurveTo(h, -h, h - radius, -h);
  shape.lineTo(-h + radius, -h);
  shape.quadraticCurveTo(-h, -h, -h, -h + radius);

  return new THREE.ShapeGeometry(shape);
}

// ShapeGeometry는 기본적으로 세워져 있습니다. 아예 태어날 때부터 눕혀버리는 게 제일 속 편합니다.
const ROUNDED_GEOMETRY = createRoundedRectGeometry(
  VOXEL_CONFIG.SIZE * 0.98,
  0.8,
);
// 🚀 생성 직후에 X축으로 90도 눕힙니다.
ROUNDED_GEOMETRY.rotateX(-Math.PI / 2);

const MAX_INSTANCES = 1500;

export function UserMarker() {
  const { devicePos, accuracy, originOffset, themeKey } = useMapNavigation();

  // 1. 테마 정의
  const targetTheme = MAP_THEMES[themeKey];
  const isNight = themeKey === 'night';
  const themeAccent = targetTheme.grid.section;
  const gemColor = isNight ? '#00f2ff' : '#ff1493';

  // 2. 내 로컬 위치 (Floating Origin 적용)
  const localPos = useMemo(() => {
    if (!devicePos) return null;
    return {
      x: devicePos.x - originOffset.x,
      y: 0, // 바닥에 붙임
      z: devicePos.z - originOffset.z,
    };
  }, [devicePos, originOffset]);

  // 3. 격자 스냅 위치 계산 (snapToVoxel은 절대 좌표 기반이므로 그대로 유지 후 훅 적용)
  const snappedLocalPos = useMemo(() => {
    if (!devicePos) return null;
    const snappedAbs = VOXEL_CONFIG.snapToVoxel(devicePos);
    return {
      x: snappedAbs.x - originOffset.x,
      z: snappedAbs.z - originOffset.z,
    };
  }, [devicePos, originOffset]);

  // 4. 보석의 상대 위치 (중앙 정렬 보정)
  const gemOffset = useMemo(() => {
    if (!localPos || !snappedLocalPos) return { x: 0, z: 0 };
    return {
      x: localPos.x - VOXEL_CONFIG.toCenter(snappedLocalPos.x),
      z: localPos.z - VOXEL_CONFIG.toCenter(snappedLocalPos.z),
    };
  }, [localPos, snappedLocalPos]);

  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const tempObject = useMemo(() => new THREE.Object3D(), []);

  useFrame((state) => {
    if (!gemRef.current || !meshRef.current || !localPos) return;

    // 가시성 로직 (기존 유지)
    const markerWorldPos = new THREE.Vector3();
    gemRef.current.getWorldPosition(markerWorldPos);
    const distance = state.camera.position.distanceTo(markerWorldPos);
    const visibility = Math.exp(-Math.pow(distance * FOG_DENSITY, 2));

    const isVisible = visibility > 0.01;
    gemRef.current.visible = isVisible;
    meshRef.current.visible = isVisible;

    // 안개가 완전히 가리는 FADE_END * 1.5 너머라면 아예 보이지 않게 처리
    if (distance > THREE_CONFIG.RENDERING.FADE_END * 1.5) {
      gemRef.current.visible = false;
      meshRef.current.visible = false;
      return;
    }

    // 애니메이션
    const t = state.clock.getElapsedTime();
    gemRef.current.rotation.y = t * 1.2;
    gemRef.current.position.y = 2 + Math.sin(t * 1.5) * 0.25;
  });

  // gridOffsets 계산 (기존 유지)
  const gridOffsets = useMemo(() => {
    const activeAccuracy = accuracy || 10;
    const safeAccuracy = Math.min(activeAccuracy, 50);
    const offsets = [];
    const step = VOXEL_CONFIG.SIZE;
    const range = Math.ceil(safeAccuracy / step);

    for (let x = -range; x <= range; x++) {
      for (let z = -range; z <= range; z++) {
        const distanceSq = (x * step) ** 2 + (z * step) ** 2;
        if (distanceSq <= safeAccuracy ** 2) {
          offsets.push({ x: x * step, z: z * step });
        }
      }
    }
    return offsets.slice(0, MAX_INSTANCES);
  }, [accuracy]);

  useEffect(() => {
    if (!meshRef.current) return;
    meshRef.current.instanceColor = null;
    for (let i = 0; i < MAX_INSTANCES; i++) {
      tempObject.position.set(0, -1000, 0);
      tempObject.updateMatrix();
      meshRef.current.setMatrixAt(i, tempObject.matrix);
    }
    gridOffsets.forEach((offset, i) => {
      tempObject.position.set(offset.x, 0.125, offset.z); // y는 0에 가깝게
      // tempObject.rotation.set(-Math.PI / 2, 0, 0); // 🚀 지오메트리를 이미 눕혔다면 이건 삭제!
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(i, tempObject.matrix);
    });
    meshRef.current.instanceMatrix.needsUpdate = true;
    meshRef.current.count = gridOffsets.length;
  }, [gridOffsets, tempObject]);

  if (!snappedLocalPos || !localPos) return null;

  return (
    <group
      position={[
        VOXEL_CONFIG.toCenter(snappedLocalPos.x),
        VOXEL_CONFIG.SYSTEM_OFFSET, // 🚀 0.25m 시스템 층 안착
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

      <pointLight
        position={[gemOffset.x, 2, gemOffset.z]}
        distance={16} // 15보다 16(2^4)이 더 기분 좋네요!
        intensity={isNight ? 10 : 3}
        color={gemColor}
      />
    </group>
  );
}
