import { useMemo, useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber'; // 🚀 애니메이션을 위해 추가
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapStore } from '../../model/map-store';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';

const { GRID_SIZE } = projection;
const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

function createRoundedRectGeometry(size: number, radius: number) {
  const shape = new THREE.Shape();
  const x = -size / 2;
  const y = -size / 2;

  shape.moveTo(x, y + radius);
  shape.lineTo(x, y + size - radius);
  shape.quadraticCurveTo(x, y + size, x + radius, y + size);
  shape.lineTo(x + size - radius, y + size);
  shape.quadraticCurveTo(x + size, y + size, x + size, y + size - radius);
  shape.lineTo(x + size, y + radius);
  shape.quadraticCurveTo(x + size, y, x + size - radius, y);
  shape.lineTo(x + radius, y);
  shape.quadraticCurveTo(x, y, x, y + radius);

  return new THREE.ShapeGeometry(shape);
}

const ROUNDED_GEOMETRY = createRoundedRectGeometry(GRID_SIZE * 0.98, 0.8);
const MAX_INSTANCES = 1500;

export function UserMarker() {
  const { location, accuracy } = useDeviceLocationStore();
  const { project } = useMapStore();

  // 🚀 [추가] 오프셋 구독
  const originOffset = useMapStore((state) => state.originOffset);

  // 내 실제 위치 (절대 좌표)
  const absPosition = useMemo(
    () => (location ? project(location) : null),
    [location, project],
  );

  /**
   * 🚀 [보정] 내 로컬 위치 계산
   * 절대 좌표에서 현재 월드의 오프셋을 뺍니다.
   */
  const localPosition = useMemo(() => {
    if (!absPosition) return null;
    return {
      x: absPosition.x - originOffset.x,
      y: absPosition.y - originOffset.y,
      z: absPosition.z - originOffset.z,
    };
  }, [absPosition, originOffset]);

  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null); // 🚀 보석 애니메이션을 위한 ref
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

  const gridOffsets = useMemo(() => {
    const activeAccuracy = accuracy || 10;
    const safeAccuracy = Math.min(activeAccuracy, 50);
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

  if (!localPosition) return null;

  return (
    // 🚀 보정된 로컬 좌표를 적용합니다.
    <group position={[localPosition.x, localPosition.y, localPosition.z]}>
      {/* 바닥 정확도 그리드 */}
      <instancedMesh
        ref={meshRef}
        args={[ROUNDED_GEOMETRY, undefined, MAX_INSTANCES]}
        frustumCulled={false}
      >
        <meshBasicMaterial
          color="#AEC6FF"
          transparent
          opacity={0.4}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </instancedMesh>

      {/* 💎 보석 마커: 회전하며 반짝임 */}
      <mesh
        ref={gemRef}
        position={[0, 2, 0]}
        scale={[2.5, 2.5, 2.5]}
        frustumCulled={false} // 🚀 확대 시 사라짐 방지 핵심
      >
        <octahedronGeometry args={[0.5, 0]} />
        <meshStandardMaterial
          color="#D0E0FF"
          emissive="#AEC6FF"
          emissiveIntensity={1.0}
          flatShading
        />
      </mesh>

      {/* 보석에서 나오는 은은한 빛 */}
      <pointLight
        position={[0, 2, 0]}
        // 🚀 FOG_FAR 설정에 맞춰 빛의 영향력도 opacity와 동기화하면 더 정교해집니다.
        distance={10}
        intensity={8 * (gemRef.current?.visible ? 1 : 0)} // 마커가 꺼지면 빛도 같이 끔
        color="#AEC6FF"
      />
    </group>
  );
}
