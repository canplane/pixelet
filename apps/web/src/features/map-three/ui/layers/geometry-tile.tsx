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
import { toThreePosWithOffset } from '@/shared/lib/three/three-coords';

const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO } = OSM_CONFIG;

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

interface GeometryTileProps {
  tile: WorldTile;
}

/**
 * [GeometryTile]
 * 4개의 독립된 채널(Base, Water, Transport, Structure)을 쉐이더에서 합성하여
 * 최종 지형을 렌더링하는 핵심 컴포넌트입니다.
 */
export const GeometryTile = React.memo(({ tile }: GeometryTileProps) => {
  const { grids, origin, resolution, key } = tile;

  // 1. 절대 좌표 중심점 (지리적 표준: x=동서, y=남북, z=고도)
  const absoluteCenter = useMemo(
    () => ({
      x: origin.x + TILE_SIZE / 2,
      y: origin.y + TILE_SIZE / 2, // 지리적 Y (북쪽)
      z: 0,
    }),
    [origin.x, origin.y],
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

  const texHeight = useMemo(
    () => createDataTexture(grids.height, resolution),
    [grids.height, resolution],
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

        uHeight: { value: texHeight },

        // 🎯 물리적 스케일 상수 (2m * 배율)
        uVoxelZUnit: { value: CELL_SIZE * Z_SCALE_RATIO },

        // Colors (Palette)
        uColorUnknown: { value: new THREE.Color(VOXEL_COLORS.UNKNOWN) },
        uColorBaseLand: { value: new THREE.Color(VOXEL_COLORS.BASE_LAND) },
        uColorWater: { value: new THREE.Color(VOXEL_COLORS.WATER) },
        uColorGreenDeep: { value: new THREE.Color(VOXEL_COLORS.GREEN_DEEP) },
        uColorGreenLight: { value: new THREE.Color(VOXEL_COLORS.GREEN_LIGHT) },
        uColorSand: { value: new THREE.Color(VOXEL_COLORS.SAND) },
        uColorResidential: { value: new THREE.Color(VOXEL_COLORS.RESIDENTIAL) },
        uColorCommercial: { value: new THREE.Color(VOXEL_COLORS.COMMERCIAL) },
        uColorIndustrial: { value: new THREE.Color(VOXEL_COLORS.INDUSTRIAL) },
        uColorInstitutional: {
          value: new THREE.Color(VOXEL_COLORS.INSTITUTIONAL),
        },
        uColorSports: { value: new THREE.Color(VOXEL_COLORS.SPORTS) },
        uColorRoad: { value: new THREE.Color(VOXEL_COLORS.ROAD) },
        uColorPedestrian: { value: new THREE.Color(VOXEL_COLORS.PEDESTRIAN) },
        uColorRail: { value: new THREE.Color(VOXEL_COLORS.RAIL) },
        uColorParking: { value: new THREE.Color(VOXEL_COLORS.PARKING) },
        uColorBuilding: { value: new THREE.Color(VOXEL_COLORS.BUILDING) },
      },
      vertexShader: `
        varying vec2 vUv;
        uniform sampler2D uHeight;
        uniform float uVoxelZUnit;

        void main() {
          vUv = uv;

          // 1. 고도 데이터 샘플링 (0~255 인덱스)
          // 💡 texture2D는 WebGL 1/2 호환을 위해 유지 (texture()는 WebGL 2 전용)
          float hIndex = floor(texture2D(uHeight, vUv).r * 255.0 + 0.5);
          
          // 2. 실제 미터 높이 계산
          float height = hIndex * uVoxelZUnit;
          
          // 3. 정점 이동 (PlaneGeometry는 기본적으로 Z축 방향이 정면임)
          // 눕혀진 상태에서 '위'로 솟아야 하므로 z값을 변경합니다.
          vec3 pos = position;
          pos.z += height;

          gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
        }
      `,
      fragmentShader: `
        precision highp float;
        
        uniform sampler2D uBase;
        uniform sampler2D uWater;
        uniform sampler2D uTransport;
        uniform sampler2D uStructure;

        uniform vec3 uColorUnknown, uColorBaseLand, uColorWater;
        uniform vec3 uColorGreenDeep, uColorGreenLight, uColorSand;
        uniform vec3 uColorResidential, uColorCommercial, uColorIndustrial, uColorInstitutional, uColorSports;
        uniform vec3 uColorRoad, uColorPedestrian, uColorRail, uColorParking;
        uniform vec3 uColorBuilding;

        varying vec2 vUv;

        // Type ID 매핑 (baker의 VOXEL_TYPES와 일치해야 함)
        #define TYPE_UNKNOWN ${VOXEL_TYPES.UNKNOWN}.0
        #define TYPE_BASE_LAND ${VOXEL_TYPES.BASE_LAND}.0
        #define TYPE_WATER ${VOXEL_TYPES.WATER}.0
        #define TYPE_GREEN_DEEP ${VOXEL_TYPES.GREEN_DEEP}.0
        #define TYPE_GREEN_LIGHT ${VOXEL_TYPES.GREEN_LIGHT}.0
        #define TYPE_SAND ${VOXEL_TYPES.SAND}.0
        #define TYPE_RESIDENTIAL ${VOXEL_TYPES.RESIDENTIAL}.0
        #define TYPE_COMMERCIAL ${VOXEL_TYPES.COMMERCIAL}.0
        #define TYPE_INDUSTRIAL ${VOXEL_TYPES.INDUSTRIAL}.0
        #define TYPE_INSTITUTIONAL ${VOXEL_TYPES.INSTITUTIONAL}.0
        #define TYPE_SPORTS ${VOXEL_TYPES.SPORTS}.0
        #define TYPE_ROAD ${VOXEL_TYPES.ROAD}.0
        #define TYPE_PEDESTRIAN ${VOXEL_TYPES.PEDESTRIAN}.0
        #define TYPE_RAIL ${VOXEL_TYPES.RAIL}.0
        #define TYPE_PARKING ${VOXEL_TYPES.PARKING}.0
        #define TYPE_BUILDING ${VOXEL_TYPES.BUILDING}.0

        vec3 getTypeColor(float type) {
          if (abs(type - TYPE_WATER) < 0.1) return uColorWater;
          if (abs(type - TYPE_GREEN_DEEP) < 0.1) return uColorGreenDeep;
          if (abs(type - TYPE_GREEN_LIGHT) < 0.1) return uColorGreenLight;
          if (abs(type - TYPE_SAND) < 0.1) return uColorSand;
          if (abs(type - TYPE_RESIDENTIAL) < 0.1) return uColorResidential;
          if (abs(type - TYPE_COMMERCIAL) < 0.1) return uColorCommercial;
          if (abs(type - TYPE_INDUSTRIAL) < 0.1) return uColorIndustrial;
          if (abs(type - TYPE_INSTITUTIONAL) < 0.1) return uColorInstitutional;
          if (abs(type - TYPE_SPORTS) < 0.1) return uColorSports;
          if (abs(type - TYPE_ROAD) < 0.1) return uColorRoad;
          if (abs(type - TYPE_PEDESTRIAN) < 0.1) return uColorPedestrian;          
          if (abs(type - TYPE_RAIL) < 0.1) return uColorRail;
          if (abs(type - TYPE_PARKING) < 0.1) return uColorParking;
          if (abs(type - TYPE_BUILDING) < 0.1) return uColorBuilding;
          return uColorBaseLand;
        }

        void main() {
          /**
           * 🎯 [UV-Flip Correction]
           * Three.js Plane을 -90도 눕히면, UV.y는 월드의 -Z(북쪽) 방향을 향합니다.
           * DataTexture는 아래에서 위로 저장되므로, 
           * 지리적 북쪽(데이터 상단)이 월드의 북쪽(-Z)에 오려면 
           * 쉐이더에서 별도의 Y-Flip(1.0-y)이 필요하지 않을 수 있습니다.
           * 만약 위아래가 바뀌어 보인다면 vUv를 그대로 사용하세요.
           */
          vec2 uv = vUv;

          // 1. 샘플링 (8비트 데이터를 정수로 복원)
          float tBase = floor(texture2D(uBase, uv).r * 255.0 + 0.5);
          float tWater = floor(texture2D(uWater, uv).r * 255.0 + 0.5);
          float tTrans = floor(texture2D(uTransport, uv).r * 255.0 + 0.5);
          float tStruct = floor(texture2D(uStructure, uv).r * 255.0 + 0.5);

          // 2. 레이어링 합성 (Priority: Structure > Transport > Water > Base)
          vec3 finalColor = getTypeColor(tBase);
          if (tWater > TYPE_UNKNOWN) finalColor = getTypeColor(tWater);
          if (tTrans > TYPE_UNKNOWN) finalColor = getTypeColor(tTrans);
          if (tStruct > TYPE_UNKNOWN) finalColor = getTypeColor(tStruct);

          gl_FragColor = vec4(finalColor, 1.0);
        }
      `,
      // ✨ 바닥면이므로 깊이 연산 최적화
      transparent: false,
      depthWrite: true,
      depthTest: true,
      // ✨ 지형이 솟아오르므로 와이어프레임이나 사이드면 처리가 필요할 수 있음
      side: THREE.DoubleSide,
    });
  }, [texBase, texWater, texTransport, texStructure, texHeight]);

  return (
    <mesh
      name={`tile-${key}`}
      /**
       * 🎯 [Axis-Mapping]
       * relativePosition.x: 동서 (World X)
       * relativePosition.z: 고도 (World Y) 🎯
       * -relativePosition.y: 남북 (World -Z) 🎯
       */
      position={toThreePosWithOffset(relativePosition, -0.01)} // 지표면(Y)을 아주 살짝 낮춤
      // y축을 미세하게 내리는 처리는 축 변환 후인 월드 Y(두 번째 인자)에 적용
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      receiveShadow
    >
      {/** * 🎯 [Crucial Update]: 세그먼트 분할
       * resolution(2048) 그대로 넣으면 성능이 폭발하므로,
       * 베이킹 샘플 수(33)와 맞추거나 적절한 수준(예: 256~512)으로 타협이 필요합니다.
       * 여기서는 샘플 데이터의 정합성을 위해 (sampleCount - 1)인 32를 기준으로 봅니다.
       */}
      <planeGeometry args={[TILE_SIZE, TILE_SIZE, 32, 32]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
});
