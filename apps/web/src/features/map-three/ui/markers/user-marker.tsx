import { useMemo, useEffect, useRef, type ReactElement } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { CoordOps } from '@pixelet/shared/shared/model/coords';
import {
  snapToCell,
  toCenter,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { FloatingGroup } from '@/entities/osm/ui/floating-group';
import { useTerrainAltitude } from '@/entities/osm/lib/use-terrain-altitude';
import {
  toThreeCoord,
  toThreeCoordWithOffset,
} from '@/shared/lib/three/three-coords';

const { CELL_SIZE } = OSM_CONSTANTS;
const MAX_INSTANCES = 1500;

// ⚙️ [Config]
const CONFIG = {
  SCALE: 1.001,
  TERRAIN_OFFSET: -CELL_SIZE,
  RADIUS_M: 16,
};

// 🎨 [Style]
const STYLE = {
  COLOR: '#4fd1c5',
  FILL_OPACITY: 0.4,
  BORDER_WIDTH: 0.085,
};

export function UserMarker(): ReactElement | null {
  const { devicePos, themeKey } = useMapNavigation();
  const { getTerrainAltitude } = useTerrainAltitude();

  // Refs
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const gemRef = useRef<THREE.Mesh>(null);
  const tempObject = useMemo(() => new THREE.Object3D(), []);

  // Theme
  const isNight = themeKey === 'night';
  const gemColor = isNight ? '#00f2ff' : '#ff1493';

  // 1️⃣ [Center]
  const snappedCenter = useMemo(() => {
    if (!devicePos) return { x: 0, y: 0 };
    return toCenter(snapToCell(devicePos));
  }, [devicePos]);

  // 2️⃣ [Height]
  const centerSurfaceH = useMemo(() => {
    return devicePos ? getTerrainAltitude(devicePos) : 0;
  }, [devicePos, snappedCenter, getTerrainAltitude]);

  // 3️⃣ [Offset]
  const gemOffset = useMemo(() => {
    if (!devicePos) return { x: 0, y: 0 };
    return CoordOps.sub(devicePos, snappedCenter);
  }, [devicePos, snappedCenter]);

  // 4️⃣ [Grid Logic] 둥근 모양 & 외곽선 끊김 완벽 해결
  const gridData = useMemo(() => {
    if (!devicePos) return [];

    const radius = CONFIG.RADIUS_M;

    // 🎯 [Rounder Shape] 반경 축소 유지 (둥글게)
    const effectiveRadius = radius - 0.8;
    const radiusSq = effectiveRadius * effectiveRadius;

    // 🛠️ [Border Fix]
    // 별도의 borderCheckRadiusSq를 만들지 않고 radiusSq를 그대로 씁니다.
    // 원칙: "내 옆 칸이 렌더링에서 탈락했다면, 무조건 테두리를 그린다."

    const step = CELL_SIZE;
    const range = Math.ceil(radius / step);

    const result: {
      x: number;
      y: number;
      borders: [number, number, number, number];
    }[] = [];

    for (let x = -range; x <= range; x++) {
      for (let y = -range; y <= range; y++) {
        const dx = x * step;
        const dy = y * step;
        const currentDistSq = dx * dx + dy * dy;

        // 렌더링 되는 셀에 대해서만 계산
        if (currentDistSq <= radiusSq) {
          // 각 방향의 이웃이 반경(radiusSq)을 벗어났는지 체크
          // 벗어났다면(true) -> 테두리 그림(1.0)

          const dTop = dx * dx + (dy + step) * (dy + step);
          const bTop = dTop > radiusSq ? 1.0 : 0.0;

          const dBottom = dx * dx + (dy - step) * (dy - step);
          const bBottom = dBottom > radiusSq ? 1.0 : 0.0;

          const dRight = (dx + step) * (dx + step) + dy * dy;
          const bRight = dRight > radiusSq ? 1.0 : 0.0;

          const dLeft = (dx - step) * (dx - step) + dy * dy;
          const bLeft = dLeft > radiusSq ? 1.0 : 0.0;

          result.push({
            x: dx,
            y: dy,
            borders: [bTop, bBottom, bRight, bLeft],
          });
        }
      }
    }
    return result.slice(0, MAX_INSTANCES);
  }, [devicePos]);

  // 5️⃣ [Shader] Material (변동 없음)
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any,
        uniforms: {
          uColor: { value: new THREE.Color(STYLE.COLOR) },
          uFillOpacity: { value: STYLE.FILL_OPACITY },
          uBorderWidth: { value: STYLE.BORDER_WIDTH },
        },
        vertexShader: `
      attribute vec4 aBorders;
      varying vec2 vUv;
      varying vec4 vBorders;

      void main() {
        vUv = uv;
        vBorders = aBorders;
        gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
      }
    `,
        fragmentShader: `
      uniform vec3 uColor;
      uniform float uFillOpacity;
      uniform float uBorderWidth;
      
      varying vec2 vUv;
      varying vec4 vBorders;

      float aastep(float threshold, float value) {
        float afwidth = fwidth(value) * 0.5;
        return smoothstep(threshold - afwidth, threshold + afwidth, value);
      }

      void main() {
        vec4 baseColor = vec4(uColor, uFillOpacity);
        vec4 borderColor = vec4(uColor, 1.0);

        float borderTop    = aastep(1.0 - uBorderWidth, vUv.y) * vBorders.x;
        float borderBottom = (1.0 - aastep(uBorderWidth, vUv.y)) * vBorders.y;
        float borderRight  = aastep(1.0 - uBorderWidth, vUv.x) * vBorders.z;
        float borderLeft   = (1.0 - aastep(uBorderWidth, vUv.x)) * vBorders.w;

        float borderStrength = max(max(borderTop, borderBottom), max(borderLeft, borderRight));

        gl_FragColor = mix(baseColor, borderColor, borderStrength);
      }
    `,
        transparent: true,
        depthWrite: false,
        depthTest: true,
        side: THREE.DoubleSide,
        polygonOffset: true,
        polygonOffsetFactor: -1.0,
        polygonOffsetUnits: -4.0,
      }),
    [],
  );

  // 6️⃣ [Update] (변동 없음)
  useEffect(() => {
    if (!meshRef.current || !devicePos) return;

    const count = gridData.length;
    meshRef.current.count = count;

    const aBorders = meshRef.current.geometry.attributes
      .aBorders as THREE.InstancedBufferAttribute;
    const borderArray = aBorders.array as Float32Array;

    gridData.forEach((data, i) => {
      const absX = snappedCenter.x + data.x;
      const absY = snappedCenter.y + data.y;
      const cellH = getTerrainAltitude({ x: absX, y: absY }) ?? 0;

      const instanceAbsZ = cellH + CONFIG.TERRAIN_OFFSET + CELL_SIZE / 2;
      const centerAbsZ = centerSurfaceH + CONFIG.TERRAIN_OFFSET;
      const localZ = instanceAbsZ - centerAbsZ;

      const pos = toThreeCoordWithOffset({ x: data.x, y: data.y }, localZ);

      tempObject.position.set(...pos);
      tempObject.scale.set(
        CELL_SIZE * CONFIG.SCALE,
        CELL_SIZE * CONFIG.SCALE,
        CELL_SIZE * CONFIG.SCALE,
      );
      tempObject.updateMatrix();
      meshRef.current!.setMatrixAt(i, tempObject.matrix);

      borderArray[i * 4 + 0] = data.borders[0];
      borderArray[i * 4 + 1] = data.borders[1];
      borderArray[i * 4 + 2] = data.borders[2];
      borderArray[i * 4 + 3] = data.borders[3];
    });

    meshRef.current.instanceMatrix.needsUpdate = true;
    aBorders.needsUpdate = true;
  }, [gridData, snappedCenter, centerSurfaceH, getTerrainAltitude, tempObject]);

  // 7️⃣ [Animation] Gem Floating (변동 없음)
  useFrame((state) => {
    if (!devicePos || !gemRef.current) return;

    const markerWorldPos = new THREE.Vector3();
    gemRef.current.getWorldPosition(markerWorldPos);
    const distance = state.camera.position.distanceTo(markerWorldPos);
    const isVisible = distance < THREE_CONFIG.RENDERING.FOG_FAR;

    gemRef.current.visible = isVisible;
    if (meshRef.current) meshRef.current.visible = isVisible;

    if (isVisible) {
      const t = state.clock.getElapsedTime();
      gemRef.current.rotation.y = t * 1.2;

      // Gem 높이: 복셀보다 확실히 위(1.5)
      const baseHeight = CELL_SIZE * 1.5;
      gemRef.current.position.y = baseHeight + Math.sin(t * 2) * 0.2;
    }
  });

  if (!devicePos) return null;

  const groupBaseHeight = centerSurfaceH + CONFIG.TERRAIN_OFFSET;

  const gemPos = toThreeCoordWithOffset({ x: gemOffset.x, y: gemOffset.y }, 0);
  const lightPos = toThreeCoord({ x: gemOffset.x, y: gemOffset.y });

  return (
    <FloatingGroup coord={{ ...snappedCenter, z: groupBaseHeight }}>
      <instancedMesh
        ref={meshRef}
        args={[
          useMemo(() => {
            const geo = new THREE.BoxGeometry(1, 1, 1);
            geo.setAttribute(
              'aBorders',
              new THREE.InstancedBufferAttribute(
                new Float32Array(MAX_INSTANCES * 4),
                4,
              ),
            );
            return geo;
          }, []),
          material,
          MAX_INSTANCES,
        ]}
        renderOrder={5}
      />

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
