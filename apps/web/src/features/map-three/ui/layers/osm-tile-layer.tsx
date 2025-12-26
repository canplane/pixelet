import React, { useMemo } from 'react';
import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { useRelativePosition } from '../../model/use-relative-position';
import type { WorldTile } from '@pixelet/shared/entities/osm/model/tiles';
import { THREE_CONFIG } from '../../model/three-config';

const { TILE_SIZE } = OSM_CONFIG;

interface OsmTileLayerProps {
  tile: WorldTile;
}

/**
 * [Helper] DataTexture 생성 팩토리
 * Uint8Array 데이터를 GPU가 효율적으로 읽을 수 있는 단일 채널(R8) 텍스처로 변환합니다.
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
  tex.magFilter = THREE.NearestFilter; // 복셀 경계를 뚜렷하게 유지
  tex.minFilter = THREE.NearestFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
};

/**
 * [osm-tile-layer]
 * 4개의 독립된 채널(Base, Water, Transport, Structure)을 쉐이더에서 합성하여
 * 최종 지형을 렌더링하는 핵심 컴포넌트입니다.
 */
export const OsmTileLayer = React.memo(({ tile }: OsmTileLayerProps) => {
  const { grids, origin, resolution, key } = tile;

  // 1. 절대 좌표 중심점 계산
  const absoluteCenter = useMemo(
    () => ({
      x: origin.x + TILE_SIZE / 2,
      y: 0,
      z: origin.z + TILE_SIZE / 2,
    }),
    [origin.x, origin.z, TILE_SIZE],
  );

  // 2. 렌더링용 상대 좌표 변환 (Floating Origin 대응)
  const relativePosition = useRelativePosition(absoluteCenter);

  // 3. [Multi-Channel] 텍스처 생성
  // 메모리 효율을 위해 각 레이어는 8비트 단일 채널로 유지됩니다.
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
   * CPU의 연산 부하를 GPU로 위임하여 실시간으로 4개의 레이어를 합성합니다.
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

        // ✨ [New] 구역 전용 색상
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

        uniform vec3 uColorEmpty, uColorWater, uColorRoad, uColorBuilding;
        uniform vec3 uColorGreenDeep, uColorGreenLight, uColorBaseLand;
        uniform vec3 uColorResidential, uColorCommercial, uColorIndustrial;
        uniform vec3 uColorSand, uColorRail, uColorPedestrian;
        uniform vec3 uColorInstitutional, uColorParking, uColorSports;

        varying vec2 vUv;

        // Type ID 매핑 (baker의 VOXEL_TYPES와 일치해야 함)
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
        #define TYPE_INSTITUTIONAL ${VOXEL_TYPES.INSTITUTIONAL}.0
        #define TYPE_PARKING ${VOXEL_TYPES.PARKING}.0
        #define TYPE_SPORTS ${VOXEL_TYPES.SPORTS}.0

        vec3 getTypeColor(float type) {
          if (abs(type - TYPE_WATER) < 0.1) return uColorWater;
          if (abs(type - TYPE_ROAD) < 0.1) return uColorRoad;
          if (abs(type - TYPE_BUILDING) < 0.1) return uColorBuilding;
          if (abs(type - TYPE_GREEN_DEEP) < 0.1) return uColorGreenDeep;
          if (abs(type - TYPE_GREEN_LIGHT) < 0.1) return uColorGreenLight;
          if (abs(type - TYPE_SAND) < 0.1) return uColorSand;
          if (abs(type - TYPE_RESIDENTIAL) < 0.1) return uColorResidential;
          if (abs(type - TYPE_COMMERCIAL) < 0.1) return uColorCommercial;
          if (abs(type - TYPE_INDUSTRIAL) < 0.1) return uColorIndustrial;
          if (abs(type - TYPE_PEDESTRIAN) < 0.1) return uColorPedestrian;
          if (abs(type - TYPE_RAIL) < 0.1) return uColorRail;
          if (abs(type - TYPE_INSTITUTIONAL) < 0.1) return uColorInstitutional;
          if (abs(type - TYPE_PARKING) < 0.1) return uColorParking;
          if (abs(type - TYPE_SPORTS) < 0.1) return uColorSports;
          return uColorBaseLand;
        }

        void main() {
          // UV Y-Flip Correction (DataTexture와 Plane UV 정렬)
          vec2 uv = vec2(vUv.x, 1.0 - vUv.y); 

          // 1. 샘플링 (8비트 데이터를 정수로 복원)
          float tBase = floor(texture2D(uBase, uv).r * 255.0 + 0.5);
          float tWater = floor(texture2D(uWater, uv).r * 255.0 + 0.5);
          float tTrans = floor(texture2D(uTransport, uv).r * 255.0 + 0.5);
          float tStruct = floor(texture2D(uStructure, uv).r * 255.0 + 0.5);

          // 2. 레이어링 합성 (Priority: Structure > Transport > Water > Base)
          vec3 finalColor = getTypeColor(tBase);
          if (tWater > TYPE_EMPTY) finalColor = getTypeColor(tWater);
          if (tTrans > TYPE_EMPTY) finalColor = getTypeColor(tTrans);
          if (tStruct > TYPE_EMPTY) finalColor = getTypeColor(tStruct);

          gl_FragColor = vec4(finalColor, 1.0);
        }
      `,
      // ✨ 바닥면이므로 깊이 연산 최적화
      transparent: false,
      depthWrite: true,
      depthTest: true,
    });
  }, [texBase, texWater, texTransport, texStructure]);

  return (
    <mesh
      name={`tile-${key}`}
      // 바닥 타일 간의 미세한 겹침을 방지하기 위해 y축을 아주 미세하게 조정
      position={[
        relativePosition.x,
        relativePosition.y - 0.01,
        relativePosition.z,
      ]}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      receiveShadow
    >
      <planeGeometry args={[TILE_SIZE, TILE_SIZE]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
});
