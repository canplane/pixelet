import React, { useMemo } from 'react';
import * as THREE from 'three';
import { VOXEL_TYPES } from '@/entities/osm/lib/baking/tile-baker';
import { OSM_CONFIG } from '@/entities/osm/config/osm-config';
import { VOXEL_COLORS } from '@/entities/osm/config/map-theme';
import { useRelativePosition } from '../../model/use-relative-position';
import type { WorldTile } from '../../model/map-store';
import { THREE_CONFIG } from '../../model/three-config';

interface OsmTileLayerProps {
  tile: WorldTile;
}

/**
 * [Helper] DataTexture 생성 팩토리
 */
const createDataTexture = (data: Uint8Array, resolution: number) => {
  const tex = new THREE.DataTexture(
    data,
    resolution,
    resolution,
    THREE.RedFormat,
    THREE.UnsignedByteType,
  );
  tex.internalFormat = 'R8';
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
};

/**
 * [osm-tile-layer]
 * 4개의 채널(Base, Water, Transport, Structure)을 합성하여 렌더링합니다.
 * ✨ [Update] Institutional, Parking, Sports 등 세분화된 타입 지원 추가
 */
export const OsmTileLayer = React.memo(({ tile }: OsmTileLayerProps) => {
  const { grids, origin, resolution, key } = tile;
  const TILE_SIZE = OSM_CONFIG.TILE_SIZE;

  // 1. 절대 좌표 중심점 계산
  const absoluteCenter = useMemo(
    () => ({
      x: origin.x + TILE_SIZE / 2,
      y: 0,
      z: origin.z + TILE_SIZE / 2,
    }),
    [origin.x, origin.z, TILE_SIZE],
  );

  // 2. 렌더링용 상대 좌표 변환
  const relativePosition = useRelativePosition(absoluteCenter);

  // 3. [Multi-Channel] 텍스처 생성
  const texBase = useMemo(
    () => createDataTexture(grids.base, resolution),
    [grids.base, resolution],
  );
  const texWater = useMemo(
    () => createDataTexture(grids.water, resolution),
    [grids.water, resolution],
  );
  const texTransport = useMemo(
    () => createDataTexture(grids.transport, resolution),
    [grids.transport, resolution],
  );
  const texStructure = useMemo(
    () => createDataTexture(grids.structure, resolution),
    [grids.structure, resolution],
  );

  /**
   * 4. [Shader] 레이어 합성 쉐이더
   */
  const material = useMemo(() => {
    return new THREE.ShaderMaterial({
      uniforms: {
        // Textures
        uBase: { value: texBase },
        uWater: { value: texWater },
        uTransport: { value: texTransport },
        uStructure: { value: texStructure },

        // Colors (Palette)
        uColorEmpty: { value: new THREE.Color(VOXEL_COLORS.EMPTY) },
        uColorWater: { value: new THREE.Color(VOXEL_COLORS.WATER) },
        uColorRoad: { value: new THREE.Color(VOXEL_COLORS.ROAD) },
        uColorBuilding: { value: new THREE.Color(VOXEL_COLORS.BUILDING) },
        uColorGreenDeep: { value: new THREE.Color(VOXEL_COLORS.GREEN_DEEP) },
        uColorGreenLight: { value: new THREE.Color(VOXEL_COLORS.GREEN_LIGHT) },
        uColorBaseLand: { value: new THREE.Color(VOXEL_COLORS.BASE_LAND) },
        uColorResidential: { value: new THREE.Color(VOXEL_COLORS.RESIDENTIAL) },
        uColorCommercial: { value: new THREE.Color(VOXEL_COLORS.COMMERCIAL) },
        uColorIndustrial: { value: new THREE.Color(VOXEL_COLORS.INDUSTRIAL) },
        uColorSand: { value: new THREE.Color(VOXEL_COLORS.SAND) },
        uColorRail: { value: new THREE.Color(VOXEL_COLORS.RAIL) },
        uColorPedestrian: { value: new THREE.Color(VOXEL_COLORS.PEDESTRIAN) },

        // ✨ [New Colors]
        uColorInstitutional: {
          value: new THREE.Color(VOXEL_COLORS.INSTITUTIONAL),
        },
        uColorParking: { value: new THREE.Color(VOXEL_COLORS.PARKING) },
        uColorSports: { value: new THREE.Color(VOXEL_COLORS.SPORTS) },
      },
      vertexShader: `
        varying vec2 vUv;
        void main() {
          vUv = uv;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        precision highp float;
        
        uniform sampler2D uBase;
        uniform sampler2D uWater;
        uniform sampler2D uTransport;
        uniform sampler2D uStructure;

        // Base Colors
        uniform vec3 uColorEmpty, uColorWater, uColorRoad, uColorBuilding;
        uniform vec3 uColorGreenDeep, uColorGreenLight, uColorBaseLand;
        uniform vec3 uColorResidential, uColorCommercial, uColorIndustrial;
        uniform vec3 uColorSand, uColorRail, uColorPedestrian;
        
        // ✨ New Colors
        uniform vec3 uColorInstitutional, uColorParking, uColorSports;

        varying vec2 vUv;

        // Type Constants
        #define TYPE_EMPTY ${VOXEL_TYPES.EMPTY}.0
        #define TYPE_WATER ${VOXEL_TYPES.WATER}.0
        #define TYPE_ROAD ${VOXEL_TYPES.ROAD}.0
        #define TYPE_BUILDING ${VOXEL_TYPES.BUILDING}.0
        #define TYPE_GREEN_DEEP ${VOXEL_TYPES.GREEN_DEEP}.0
        #define TYPE_GREEN_LIGHT ${VOXEL_TYPES.GREEN_LIGHT}.0
        #define TYPE_BASE_LAND ${VOXEL_TYPES.BASE_LAND}.0
        #define TYPE_RESIDENTIAL ${VOXEL_TYPES.RESIDENTIAL}.0
        #define TYPE_COMMERCIAL ${VOXEL_TYPES.COMMERCIAL}.0
        #define TYPE_INDUSTRIAL ${VOXEL_TYPES.INDUSTRIAL}.0
        #define TYPE_SAND ${VOXEL_TYPES.SAND}.0
        #define TYPE_PEDESTRIAN ${VOXEL_TYPES.PEDESTRIAN}.0
        #define TYPE_RAIL ${VOXEL_TYPES.RAIL}.0
        
        // ✨ New Constants
        #define TYPE_INSTITUTIONAL ${VOXEL_TYPES.INSTITUTIONAL}.0
        #define TYPE_PARKING ${VOXEL_TYPES.PARKING}.0
        #define TYPE_SPORTS ${VOXEL_TYPES.SPORTS}.0

        // [Helper] 타입 ID를 색상으로 변환
        vec3 getTypeColor(float type) {
          if (abs(type - TYPE_WATER) < 0.1) return uColorWater;
          if (abs(type - TYPE_ROAD) < 0.1) return uColorRoad;
          if (abs(type - TYPE_BUILDING) < 0.1) return uColorBuilding;
          
          // Nature
          if (abs(type - TYPE_GREEN_DEEP) < 0.1) return uColorGreenDeep;
          if (abs(type - TYPE_GREEN_LIGHT) < 0.1) return uColorGreenLight;
          if (abs(type - TYPE_SAND) < 0.1) return uColorSand;
          
          // Landuse
          if (abs(type - TYPE_RESIDENTIAL) < 0.1) return uColorResidential;
          if (abs(type - TYPE_COMMERCIAL) < 0.1) return uColorCommercial;
          if (abs(type - TYPE_INDUSTRIAL) < 0.1) return uColorIndustrial;
          
          // Transport
          if (abs(type - TYPE_PEDESTRIAN) < 0.1) return uColorPedestrian;
          if (abs(type - TYPE_RAIL) < 0.1) return uColorRail;

          // ✨ New Types Mapping
          if (abs(type - TYPE_INSTITUTIONAL) < 0.1) return uColorInstitutional;
          if (abs(type - TYPE_PARKING) < 0.1) return uColorParking;
          if (abs(type - TYPE_SPORTS) < 0.1) return uColorSports;

          return uColorBaseLand; // Default
        }

        void main() {
          vec2 uv = vec2(vUv.x, 1.0 - vUv.y); // UV Y-Flip Correction

          // 1. 샘플링
          float tBase = floor(texture2D(uBase, uv).r * 255.0 + 0.5);
          float tWater = floor(texture2D(uWater, uv).r * 255.0 + 0.5);
          float tTrans = floor(texture2D(uTransport, uv).r * 255.0 + 0.5);
          float tStruct = floor(texture2D(uStructure, uv).r * 255.0 + 0.5);

          // 2. 레이어링 (Priority Stacking)
          vec3 finalColor = getTypeColor(tBase); // Layer 1: Base

          if (tWater > TYPE_EMPTY) finalColor = getTypeColor(tWater);     // Layer 2
          if (tTrans > TYPE_EMPTY) finalColor = getTypeColor(tTrans);     // Layer 3
          if (tStruct > TYPE_EMPTY) finalColor = getTypeColor(tStruct);   // Layer 4

          gl_FragColor = vec4(finalColor, 1.0);
        }
      `,
      transparent: false,
      side: THREE.FrontSide,
    });
  }, [texBase, texWater, texTransport, texStructure]);

  return (
    <mesh
      name={`tile-${key}`}
      position={[relativePosition.x, relativePosition.y, relativePosition.z]}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      receiveShadow
    >
      <planeGeometry args={[TILE_SIZE, TILE_SIZE]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
});
