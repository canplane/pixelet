import { useMemo, useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapNavigation } from '../../model/use-map-navigation';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_THEMES } from '../../model/map-theme';

const { GRID_SIZE, HALF_GRID } = projection;
const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

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

const ROUNDED_GEOMETRY = createRoundedRectGeometry(GRID_SIZE * 0.98, 0.8);
const MAX_INSTANCES = 1500;

export function UserMarker() {
  const { devicePos, accuracy, originOffset, themeKey } = useMapNavigation();

  // 1. 테마 및 야간 여부 정의 (정의되지 않았던 변수 추가)
  const targetTheme = MAP_THEMES[themeKey];
  const isNight = themeKey === 'night';
  const themeAccent = targetTheme.grid.section;

  // 🚀 동화적 포인트 컬러 (낮에는 진한 핑크, 밤에는 빛나는 민트/하늘색)
  const gemColor = isNight ? '#00f2ff' : '#ff1493';

  // 1. 내 로컬 위치 (Floating Origin 적용)
  const localPosition = useMemo(() => {
    if (!devicePos) return null;
    return {
      x: devicePos.x - originOffset.x,
      y: 0, // 바닥에 붙임
      z: devicePos.z - originOffset.z,
    };
  }, [devicePos, originOffset]);

  // 2. 내가 서 있는 '격자'의 로컬 좌표 스냅 (부모 그룹용)
  const snappedLocalPos = useMemo(() => {
    if (!devicePos) return null;
    const snappedAbs = projection.snapToGrid(devicePos);
    return {
      x: snappedAbs.x - originOffset.x,
      z: snappedAbs.z - originOffset.z,
    };
  }, [devicePos, originOffset]);

  // 3. 보석의 상대 위치 (중앙 정렬 보정)
  const gemOffset = useMemo(() => {
    if (!localPosition || !snappedLocalPos) return { x: 0, z: 0 };
    return {
      x: localPosition.x - (snappedLocalPos.x + HALF_GRID),
      z: localPosition.z - (snappedLocalPos.z + HALF_GRID),
    };
  }, [localPosition, snappedLocalPos]);

  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const tempObject = useMemo(() => new THREE.Object3D(), []);

  // 🚀 보석 애니메이션 로직: 매 프레임마다 실행됩니다.
  useFrame((state) => {
    if (!gemRef.current || !meshRef.current || !localPosition) return;

    const t = state.clock.getElapsedTime();
    gemRef.current.rotation.y = t * 1.2;
    gemRef.current.position.y = 2 + Math.sin(t * 1.5) * 0.25;

    // 🚀 [수정] 월드 좌표계를 기준으로 정확한 거리 계산
    // 마커 메쉬가 실제로 렌더링되는 월드 위치를 가져옵니다.
    const markerWorldPos = new THREE.Vector3();
    gemRef.current.getWorldPosition(markerWorldPos);

    const distance = state.camera.position.distanceTo(markerWorldPos);

    // 🚀 안개 끝(1500)보다 더 여유 있게(2500) 잡아서 툭 끊기는 느낌 방지
    const opacity = THREE.MathUtils.mapLinear(
      distance,
      FOG_NEAR,
      FOG_FAR * 1.8,
      1,
      0,
    );

    const clampedOpacity = THREE.MathUtils.clamp(opacity, 0, 1);

    // 재질 적용
    const gemMat = gemRef.current.material as THREE.MeshStandardMaterial;
    gemMat.opacity = clampedOpacity;
    gemMat.transparent = true;

    const gridMat = meshRef.current.material as THREE.MeshBasicMaterial;
    gridMat.opacity = clampedOpacity * 0.4;

    // 🚀 [최적화] 완전히 안 보일 때만 끄기
    const isVisible = clampedOpacity > 0.001;
    gemRef.current.visible = isVisible;
    meshRef.current.visible = isVisible;
  });

  // 🚀 이제 gridOffsets 계산 시 accuracy를 안전하게 사용할 수 있어
  const gridOffsets = useMemo(() => {
    const activeAccuracy = accuracy || 10; // 기본값 10m
    const safeAccuracy = Math.min(activeAccuracy, 50); // 너무 넓어지지 않게 제한

    const offsets = [];
    const step = GRID_SIZE;
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
      tempObject.position.set(offset.x, 0.1, offset.z);
      tempObject.rotation.set(-Math.PI / 2, 0, 0);
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(i, tempObject.matrix);
    });

    meshRef.current.instanceMatrix.needsUpdate = true;
    meshRef.current.count = gridOffsets.length;
  }, [gridOffsets, tempObject]);

  if (!snappedLocalPos || !localPosition) return null;

  return (
    <group
      position={[
        snappedLocalPos.x + HALF_GRID,
        0.16, // 🚀 도로(0.15)보다 살짝 위로 올려서 묻히지 않게 함
        snappedLocalPos.z + HALF_GRID,
      ]}
    >
      {/* 1. 바닥 정확도 그리드 (색상을 더 선명하게) */}
      <instancedMesh
        ref={meshRef}
        args={[ROUNDED_GEOMETRY, undefined, MAX_INSTANCES]}
        renderOrder={5} // 지형보다는 위, 도로와는 비슷하게
      >
        <meshBasicMaterial
          color={isNight ? themeAccent : '#FFD700'} // 낮엔 금색 계열로 포인트
          transparent
          opacity={0.5} // 조금 더 진하게
          depthWrite={false}
        />
      </instancedMesh>

      {/* 2. 💎 보석 마커 (색상 강화) */}
      <mesh
        ref={gemRef}
        position={[gemOffset.x, 2, gemOffset.z]}
        renderOrder={20}
      >
        <octahedronGeometry args={[0.6, 0]} />
        <meshStandardMaterial
          color={gemColor} // 🚀 하얀색 대신 실제 색상 주입
          emissive={gemColor}
          emissiveIntensity={isNight ? 2 : 0.5} // 낮에는 빛나기보다 색이 보여야 함
          roughness={0.2}
          metalness={0.8}
          flatShading
        />
      </mesh>

      {/* 3. 보석 조명 (색상 동기화) */}
      <pointLight
        position={[gemOffset.x, 2, gemOffset.z]}
        distance={15}
        intensity={isNight ? 10 : 3} // 낮엔 조명을 줄여야 색이 안 날아감
        color={gemColor}
      />
    </group>
  );
}
