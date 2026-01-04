import { useMemo, useRef, useState, useEffect, type ReactElement } from 'react';
import * as THREE from 'three';
import { useThree } from '@react-three/fiber';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { useTerrainAltitude } from '@/entities/osm/lib/use-terrain-altitude';
import { FloatingGroup } from '@/entities/osm/ui/floating-group';
import { CoordOps, type Coord } from '@pixelet/core/shared/model/coords';
import { fromThreeCoord, toThreeCoord } from '@/shared/lib/three/three-coords';
import { OSM_CONSTANTS } from '@pixelet/core/entities/osm/model/osm-constants';
import {
  snapToCell,
  toCenter,
} from '@pixelet/core/entities/osm/lib/utils/tile-utils';

const { CELL_SIZE } = OSM_CONSTANTS;

// ⚙️ [Config] Raymarching 설정
const CONFIG = {
  STEP: 0.5,
  MAX_STEPS: 200,
  BIN_ITER: 6,
  MAX_DIST_SQ: 2000 ** 2,
  TERRAIN_OFFSET: -CELL_SIZE, // Logic 보정값
};

// 🎨 [Style] 하이라이트 스타일
const STYLE = {
  COLOR: '#FF9F05', // Blender Selection Orange
  FILL_OPACITY: 0.6, // 진한 내부 채움
  BORDER_WIDTH: 0.05, // 외곽선 두께
};

// ⚡ [Static] 재사용 객체 (GC 방지)
const CENTER_POINT = toThreeCoord(toCenter({ x: 0, y: 0, z: 0 }));
const VEC3 = new THREE.Vector3();

interface HoveredInfo {
  coord: Coord;
}

export function CellHighlighter(): ReactElement | null {
  const { raycaster, pointer, camera } = useThree();
  const { updateCenter, floatingOrigin, center } = useMapNavigation();
  const { getTerrainAltitude } = useTerrainAltitude();

  // Refs (Animation Loop 등에서 최신 상태 접근용)
  const stateRef = useRef({ floatingOrigin, center });
  useEffect(() => {
    stateRef.current = { floatingOrigin, center };
  }, [floatingOrigin, center]);

  const geo = useMemo(() => new THREE.BoxGeometry(1, 1, 1), []);

  // 1️⃣ [Material] High-Priority Shader Material
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any, // fwidth 사용
        uniforms: {
          uColor: { value: new THREE.Color(STYLE.COLOR) },
          uOpacity: { value: STYLE.FILL_OPACITY },
          uBorderWidth: { value: STYLE.BORDER_WIDTH },
        },
        vertexShader: `
            varying vec2 vUv;
            void main() {
                vUv = uv;
                gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
            }
        `,
        fragmentShader: `
            uniform vec3 uColor;
            uniform float uOpacity;
            uniform float uBorderWidth;
            varying vec2 vUv;

            // 🖌️ Anti-aliased Step
            float aastep(float threshold, float value) {
                float afwidth = fwidth(value) * 0.5;
                return smoothstep(threshold - afwidth, threshold + afwidth, value);
            }

            void main() {
                // UV 기준 4방향 테두리 계산 (Anti-aliasing 적용)
                float bx = aastep(uBorderWidth, vUv.x) * aastep(vUv.x, 1.0 - uBorderWidth);
                float by = aastep(uBorderWidth, vUv.y) * aastep(vUv.y, 1.0 - uBorderWidth);

                // bx * by == 1.0 이면 중앙(Center), 아니면 테두리(Border)
                float isCenter = bx * by;
                float isBorder = 1.0 - isCenter;

                // 테두리는 1.0(불투명), 내부는 uOpacity
                float alpha = mix(uOpacity, 1.0, isBorder);
                gl_FragColor = vec4(uColor, alpha);
            }
        `,
        transparent: true,
        side: THREE.DoubleSide,
        depthTest: true,
        // 🛡️ [Priority] 가장 중요함: UserMarker(-1.0)보다 더 강하게 당김 (-2.0)
        // 이 설정 덕분에 마커와 겹칠 때 항상 이 하이라이터가 위에 그려집니다.
        polygonOffset: true,
        polygonOffsetFactor: -2.0,
        polygonOffsetUnits: -8.0,
      }),
    [],
  );

  const [hovered, setHovered] = useState<HoveredInfo | null>(null);

  // 2️⃣ [Raymarching] 지형 충돌 감지 (Logic)
  const getHeightDiff = (ray: THREE.Ray, t: number) => {
    ray.at(t, VEC3);
    const local = fromThreeCoord([VEC3.x, VEC3.y, VEC3.z]);
    const global = CoordOps.add(local, stateRef.current.floatingOrigin);
    const h = getTerrainAltitude(global) ?? 0;

    // 시각적 높이 계산: h(Data) + Offset - OriginZ
    const visualY =
      h + CONFIG.TERRAIN_OFFSET - stateRef.current.floatingOrigin.z;
    return VEC3.y - visualY;
  };

  const getIntersection = (): THREE.Vector3 | null => {
    raycaster.setFromCamera(pointer, camera);
    const ray = raycaster.ray;

    // A. Linear Search (Rough)
    let t = 0,
      hitT = -1;
    for (let i = 0; i < CONFIG.MAX_STEPS; i++, t += CONFIG.STEP) {
      if (getHeightDiff(ray, t) < 0.5) {
        // 0.5는 판정 여유값(Tolerance)
        hitT = t;
        break;
      }
    }
    if (hitT === -1) return null;

    // B. Binary Search (Refined)
    let min = hitT - CONFIG.STEP,
      max = hitT;
    for (let i = 0; i < CONFIG.BIN_ITER; i++) {
      const mid = (min + max) * 0.5;
      getHeightDiff(ray, mid) < 0.5 ? (max = mid) : (min = mid);
    }
    return ray.at(max, new THREE.Vector3());
  };

  const onMove = (e: any) => {
    e.stopPropagation();
    const hit = getIntersection();
    if (!hit) return setHovered(null);

    const local = fromThreeCoord([hit.x, hit.y, hit.z]);
    const global = CoordOps.add(local, stateRef.current.floatingOrigin);

    // 🎯 Snap to Corner Logic
    const snapped = snapToCell(global);
    const h = getTerrainAltitude(snapped) ?? 0;
    const z = h + CONFIG.TERRAIN_OFFSET;

    if (hovered?.coord.x !== snapped.x || hovered?.coord.y !== snapped.y) {
      setHovered({ coord: { ...snapped, z } });
    }
  };

  const onClick = (e: any) => {
    e.stopPropagation();
    if (!hovered) return;
    if (!stateRef.current.center) return;

    const { x, y } = CoordOps.sub(hovered.coord, stateRef.current.center);
    if (x * x + y * y <= CONFIG.MAX_DIST_SQ) {
      updateCenter(hovered.coord);
    }
  };

  return (
    <group>
      {/* Input Layer (Invisible Plane) */}
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        onClick={onClick}
        onPointerMove={onMove}
        onPointerOut={() => setHovered(null)}
      >
        <planeGeometry args={[1e5, 1e5]} />
        <meshBasicMaterial visible={false} />
      </mesh>

      {/* Visual Layer (Highlighter) */}
      {hovered && (
        <FloatingGroup coord={hovered.coord}>
          <group position={CENTER_POINT}>
            <mesh
              geometry={geo}
              material={material}
              scale={[CELL_SIZE, CELL_SIZE, CELL_SIZE]} // 정사이즈 사용 (Offset으로 해결)
              renderOrder={9999}
            />
          </group>
        </FloatingGroup>
      )}
    </group>
  );
}
