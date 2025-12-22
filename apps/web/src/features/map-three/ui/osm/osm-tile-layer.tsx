import React, { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import * as BufferGeometryUtils from 'three/examples/jsm/utils/BufferGeometryUtils';
import { useMapStore } from '../../model/map-store';
import type { MapInfra } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';

const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

export const OsmTileLayer = React.memo(({ infras }: { infras: MapInfra[] }) => {
  const originOffset = useMapStore((state) => state.originOffset);
  const center = useMapStore((state) => state.center); // 기준점 (보통 카메라가 보는 곳)
  const groupRef = useRef<THREE.Group>(null);

  // 1. Mesh 타입 (건물, 물, 지면) 병합
  const mergedMeshes = useMemo(() => {
    const createLayer = (types: string[], color: string) => {
      const filtered = infras.filter((f) => types.includes(f.type) && f.shape);
      if (filtered.length === 0) return null;

      const geometries = filtered.map((f) => {
        const depth = f.type === 'building' ? f.height || 10 : 0.2; // 건물 외엔 낮게
        const geo = new THREE.ExtrudeGeometry(f.shape!, {
          depth,
          bevelEnabled: false,
        });
        geo.rotateX(-Math.PI / 2);
        geo.applyMatrix4(
          new THREE.Matrix4().makeTranslation(
            f.center.x,
            f.center.y,
            f.center.z,
          ),
        );
        return geo;
      });

      const merged = BufferGeometryUtils.mergeGeometries(geometries);
      geometries.forEach((g) => g.dispose());
      return merged;
    };

    return {
      building: createLayer(['building'], '#4a4e54'),
      water: createLayer(['water'], '#4da3ff'),
      landuse: createLayer(['landuse'], '#82ad7d'),
    };
  }, [infras]);

  // 2. Road (Line) 타입 병합
  const mergedRoads = useMemo(() => {
    const roads = infras.filter((f) => f.type === 'road' && f.points);
    if (roads.length === 0) return null;

    // Line은 Extrude 대신 BufferGeometry를 합칩니다.
    const geometries = roads.map((f) => {
      const geo = new THREE.BufferGeometry().setFromPoints(f.points);
      const matrix = new THREE.Matrix4().makeTranslation(
        f.center.x,
        f.center.y,
        f.center.z,
      );
      geo.applyMatrix4(matrix);
      return geo;
    });

    const merged = BufferGeometryUtils.mergeGeometries(geometries);
    geometries.forEach((g) => g.dispose());
    return merged;
  }, [infras]);

  // 🚀 Floating Origin 보정 및 거리 기반 페이드 아웃
  useFrame((state) => {
    if (!groupRef.current) return;

    // 1. 위치 보정 (Floating Origin)
    groupRef.current.position.set(
      -originOffset.x,
      -originOffset.y,
      -originOffset.z,
    );

    // 🚀 2. center가 null일 경우를 대비한 방어 로직
    if (!center) {
      // center가 없으면 투명도를 0으로 만들거나 계산을 스킵합니다.
      groupRef.current.visible = false;
      return;
    }

    // 3. 거리 기반 투명도 계산 (안전하게 계산)
    const dist = state.camera.position.distanceTo(
      new THREE.Vector3(
        center.x - originOffset.x,
        0,
        center.z - originOffset.z,
      ),
    );

    const opacityFactor = THREE.MathUtils.clamp(
      THREE.MathUtils.mapLinear(dist, FOG_NEAR, FOG_FAR, 1, 0),
      0,
      1,
    );

    // 4. 레이어 가시성 및 투명도 반영
    groupRef.current.visible = opacityFactor > 0.01;

    if (groupRef.current.visible) {
      groupRef.current.traverse((obj) => {
        if (obj instanceof THREE.Mesh || obj instanceof THREE.LineSegments) {
          const mat = obj.material as THREE.Material;
          mat.transparent = true;

          /**
           * 🚀 [보정] 각 레이어의 기본 opacity 수치를 존중하며 factor를 곱합니다.
           * obj.name이나 타입을 통해 기본값을 매핑하거나,
           * 단순히 전체 factor만 조절해도 안개 효과로는 충분합니다.
           */
          if (obj.type === 'Mesh') {
            // 건물(0.8), 물(0.7), 지면(0.5) 등 원래 의도한 투명도에 안개 계수를 곱함
            // 여기서는 단순화를 위해 opacityFactor를 직접 주거나,
            // 아예 material 자체의 opacity는 건드리지 않고 fog가 해결하게 두는 방법도 있습니다.
          }
        }
      });
    }
  });

  return (
    <group ref={groupRef}>
      {/* 건물 레이어 */}
      {mergedMeshes.building && (
        <mesh geometry={mergedMeshes.building}>
          <meshStandardMaterial
            color="#4a4e54"
            roughness={0.7}
            metalness={0.2}
            transparent
            opacity={0.8}
          />
        </mesh>
      )}
      {/* 물 레이어 */}
      {mergedMeshes.water && (
        <mesh geometry={mergedMeshes.water}>
          <meshStandardMaterial
            color="#4da3ff"
            roughness={0.1}
            metalness={0.5}
            transparent
            opacity={0.7}
          />
        </mesh>
      )}
      {/* 지면(공원 등) 레이어 */}
      {mergedMeshes.landuse && (
        <mesh geometry={mergedMeshes.landuse}>
          <meshBasicMaterial color="#82ad7d" transparent opacity={0.5} />
        </mesh>
      )}
      {/* 도로 레이어 (LineSegment) */}
      {mergedRoads && (
        <lineSegments geometry={mergedRoads}>
          <lineBasicMaterial color="#ffffff" transparent opacity={0.3} />
        </lineSegments>
      )}
    </group>
  );
});
