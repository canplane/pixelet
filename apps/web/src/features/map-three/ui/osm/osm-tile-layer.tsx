import React, { useMemo } from 'react';
import * as THREE from 'three';
import * as BufferGeometryUtils from 'three/examples/jsm/utils/BufferGeometryUtils';
import { useMapNavigation } from '../../model/use-map-navigation';
import type { MapInfra } from '../../lib/osm-parser';

// 🌈 동화속 마을을 위한 파스텔 건물 팔레트
const BUILDING_COLORS = [
  '#FFD966',
  '#FFADAD',
  '#9BF6FF',
  '#CAFFBF',
  '#FFC6FF',
  '#FDFFB6',
];

const ROAD_STYLES: Record<string, { width: number; opacity: number }> = {
  motorway: { width: 4.5, opacity: 0.8 },
  trunk: { width: 4.0, opacity: 0.7 },
  primary: { width: 3.5, opacity: 0.6 },
  secondary: { width: 2.5, opacity: 0.5 },
  tertiary: { width: 2.0, opacity: 0.4 },
  residential: { width: 1.5, opacity: 0.3 },
  service: { width: 1.0, opacity: 0.2 },
};

export const OsmTileLayer = React.memo(({ infras }: { infras: MapInfra[] }) => {
  const { originOffset, themeKey } = useMapNavigation();
  const isNight = themeKey === 'night';

  const layerColors = useMemo(
    () => ({
      landuse: isNight ? '#2D4C2D' : '#A8D672',
      water: isNight ? '#1A365D' : '#7BC8F6',
      road: isNight ? '#334155' : '#FFFFFF',
    }),
    [isNight],
  );

  const terrainMeshes = useMemo(() => {
    const createLayer = (types: string[], depth: number, yPos: number) => {
      const filtered = infras.filter((f) => types.includes(f.type) && f.shape);
      if (filtered.length === 0) return null;

      const geometries = filtered.map((f) => {
        // 🚀 ShapeGeometry 대신 ExtrudeGeometry를 사용하여 면을 꽉 채우고 두께를 줍니다.
        const geo = new THREE.ExtrudeGeometry(f.shape!, {
          depth: depth, // 0.1 정도의 두께가 있어야 면이 확실히 보입니다.
          bevelEnabled: false,
        });
        geo.rotateX(-Math.PI / 2);
        geo.applyMatrix4(
          new THREE.Matrix4().makeTranslation(f.center.x, yPos, f.center.z),
        );
        return geo;
      });

      const merged = BufferGeometryUtils.mergeGeometries(geometries);
      geometries.forEach((g) => g.dispose());
      return merged;
    };

    return {
      // 🌳 땅: 가장 아래 (두께 0.1, 높이 0)
      landuse: createLayer(['landuse', 'park', 'grass'], 0.1, 0),
      // 💧 물: 땅보다 살짝 위 (두께 0.1, 높이 0.02) -> 땅 위에 파란 판이 덮임
      water: createLayer(['water'], 0.1, 0.02),
    };
  }, [infras]);

  const renderBuildings = useMemo(() => {
    return infras
      .filter((f) => f.type === 'building' && f.shape)
      .map((b, idx) => {
        const color = isNight
          ? '#4A5568'
          : BUILDING_COLORS[idx % BUILDING_COLORS.length];
        return (
          <mesh
            key={b.id}
            position={[b.center.x, 0.04, b.center.z]}
            rotation={[-Math.PI / 2, 0, 0]}
          >
            {/* 🏢 건물도 얇은 판(Extrude)으로 만들면 면적이 꽉 찹니다. */}
            <extrudeGeometry
              args={[b.shape!, { depth: 0.1, bevelEnabled: false }]}
            />
            <meshBasicMaterial color={color} />
          </mesh>
        );
      });
  }, [infras, isNight]);

  // 4. 도로 렌더링
  // 4. 도로 렌더링 부분 수정
  const renderRoads = useMemo(() => {
    return infras
      .filter((f) => f.type === 'road' && f.points.length >= 2)
      .flatMap((road) => {
        const style =
          ROAD_STYLES[road.tags.highway || 'residential'] ||
          ROAD_STYLES.residential;

        // 🚀 도로 등급에 따른 미세한 높이 차 (Z-fighting 방지)
        // 0.15를 기본으로 두고, 넓은 도로가 아주 살짝 위에 오도록 합니다.
        const ROAD_BASE_HEIGHT = 0.15;
        const layerPriority = style.width * 0.001;

        const segments = [];

        for (let i = 0; i < road.points.length - 1; i++) {
          const p1 = road.points[i];
          const p2 = road.points[i + 1];
          const len = p1.distanceTo(p2);
          const angle = Math.atan2(p2.x - p1.x, p2.z - p1.z);

          // 1. 직선 선분
          segments.push(
            <mesh
              key={`${road.id}-${i}`}
              renderOrder={10}
              position={[
                road.center.x + (p1.x + p2.x) / 2,
                0.15 + layerPriority,
                road.center.z + (p1.z + p2.z) / 2,
              ]}
              rotation={[-Math.PI / 2, 0, angle + Math.PI / 2]}
            >
              <planeGeometry args={[len, style.width]} />
              <meshBasicMaterial
                color={layerColors.road}
                transparent
                opacity={style.opacity}
                depthWrite={false}
              />
            </mesh>,
          );

          // 2. 연결부 (Joint)
          segments.push(
            <mesh
              key={`${road.id}-j-${i}`}
              renderOrder={10}
              position={[
                road.center.x + p1.x,
                0.15 + layerPriority,
                road.center.z + p1.z,
              ]}
              rotation={[-Math.PI / 2, 0, 0]}
            >
              <circleGeometry args={[style.width / 2, 8]} />
              <meshBasicMaterial
                color={layerColors.road}
                transparent
                opacity={style.opacity}
                depthWrite={false}
              />
            </mesh>,
          );
        }
        return segments;
      });
  }, [infras, layerColors.road]);

  return (
    <group position={[-originOffset.x, 0, -originOffset.z]}>
      {/* 🌳 초록 동산 땅 */}
      {terrainMeshes.landuse && (
        <mesh geometry={terrainMeshes.landuse} renderOrder={1}>
          <meshBasicMaterial color={layerColors.landuse} />
        </mesh>
      )}

      {/* 💧 물줄기/강 */}
      {terrainMeshes.water && (
        <mesh geometry={terrainMeshes.water} renderOrder={2}>
          <meshBasicMaterial
            color={layerColors.water}
            transparent
            opacity={0.9}
          />
        </mesh>
      )}

      {/* 🏢 알록달록 평면 건물들 */}
      {renderBuildings}

      {/* 🛣️ 도로망 */}
      {renderRoads}
    </group>
  );
});
