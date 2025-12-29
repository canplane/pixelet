import React, { useMemo } from 'react';
import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { useRelativePosition } from '../../model/use-relative-position';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';
import { THREE_CONFIG } from '../../model/three-config';
import { toThreePosWithOffset } from '@/shared/lib/three/three-coords';

const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO } = OSM_CONFIG;

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

const createHeightTexture = (data: Uint16Array, resolution: number) => {
  const tex = new THREE.DataTexture(
    data,
    resolution,
    resolution,
    THREE.RedIntegerFormat, // 🎯 정수 포맷 (Integer)
    THREE.UnsignedShortType,
  );
  tex.internalFormat = 'R16UI'; // 🎯 16비트 Unsigned Integer
  tex.magFilter = THREE.NearestFilter; // 🚨 정수 텍스처는 반드시 NearestFilter여야 함
  tex.minFilter = THREE.NearestFilter;
  tex.needsUpdate = true;
  return tex;
};

interface GeometryTileViewProps {
  tile: GeometryTile;
}

export const GeometryTileView = React.memo(
  ({ tile }: GeometryTileViewProps) => {
    const { grids, origin, resolution, key } = tile;
    const { layers, terrain } = grids;

    const absoluteCenter = useMemo(
      () => ({
        x: origin.x + TILE_SIZE / 2,
        y: origin.y + TILE_SIZE / 2,
        z: 0,
      }),
      [origin.x, origin.y],
    );

    const relativePosition = useRelativePosition(absoluteCenter);

    const texBase = useMemo(
      () => createDataTexture(layers.base, resolution),
      [layers.base, resolution],
    );
    const texWater = useMemo(
      () => createDataTexture(layers.water, resolution),
      [layers.water, resolution],
    );
    const texTransport = useMemo(
      () => createDataTexture(layers.transport, resolution),
      [layers.transport, resolution],
    );
    const texStructure = useMemo(
      () => createDataTexture(layers.structure, resolution),
      [layers.structure, resolution],
    );

    const texHeight = useMemo(
      () => createHeightTexture(terrain.height, resolution),
      [terrain.height, resolution],
    );

    const material = useMemo(() => {
      return new THREE.ShaderMaterial({
        // 🎯 [핵심 1] GLSL 3.00 ES 활성화 (usampler2D 사용 필수 조건)
        glslVersion: THREE.GLSL3,
        uniforms: {
          uBase: { value: texBase },
          uWater: { value: texWater },
          uTransport: { value: texTransport },
          uStructure: { value: texStructure },
          uHeight: { value: texHeight },
          uVoxelZUnit: { value: CELL_SIZE * Z_SCALE_RATIO },

          // Colors
          uColorUnknown: { value: new THREE.Color(VOXEL_COLORS.UNKNOWN) },
          uColorBaseLand: { value: new THREE.Color(VOXEL_COLORS.BASE_LAND) },
          uColorWater: { value: new THREE.Color(VOXEL_COLORS.WATER) },
          uColorGreenDeep: { value: new THREE.Color(VOXEL_COLORS.GREEN_DEEP) },
          uColorGreenLight: {
            value: new THREE.Color(VOXEL_COLORS.GREEN_LIGHT),
          },
          uColorSand: { value: new THREE.Color(VOXEL_COLORS.SAND) },
          uColorResidential: {
            value: new THREE.Color(VOXEL_COLORS.RESIDENTIAL),
          },
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
        // 🎯 [핵심 2] GLSL 3.0 문법 적용 (varying -> out, attribute -> in)
        out vec2 vUv;
        
        // 🎯 [핵심 3] 정수 텍스처 전용 샘플러 선언 (sampler2D -> usampler2D)
        uniform usampler2D uHeight; 
        uniform float uVoxelZUnit;

        void main() {
          vUv = uv;

          // 🎯 [Standard Correction]
          // 데이터는 Top-Down(이미지 표준)으로 들어오고,
          // Three.js UV는 Bottom-Up(OpenGL 표준)이므로 Y축을 뒤집어 읽습니다.
          vec2 readUv = vec2(uv.x, 1.0 - uv.y);

          // 🎯 [핵심 4] 정수 데이터 추출 (texture2D -> texture)
          // usampler2D에서 값을 읽으면 uint 타입이 반환됩니다.
          uint hIndex = texture(uHeight, readUv).r; 
          
          // uint를 float로 변환하여 높이 계산
          float height = float(hIndex) * uVoxelZUnit;
          
          vec3 pos = position;
          pos.z += height;

          gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
        }
      `,
        fragmentShader: `
        precision highp float;
        precision highp usampler2D; // 🎯 정수 샘플러 정밀도 명시

        uniform sampler2D uBase;
        uniform sampler2D uWater;
        uniform sampler2D uTransport;
        uniform sampler2D uStructure;

        // ... Uniform Colors (생략 없이 사용) ...
        uniform vec3 uColorUnknown, uColorBaseLand, uColorWater;
        uniform vec3 uColorGreenDeep, uColorGreenLight, uColorSand;
        uniform vec3 uColorResidential, uColorCommercial, uColorIndustrial, uColorInstitutional, uColorSports;
        uniform vec3 uColorRoad, uColorPedestrian, uColorRail, uColorParking;
        uniform vec3 uColorBuilding;

        in vec2 vUv; // varying -> in
        out vec4 outColor; // gl_FragColor -> out variable

        // Type ID 매핑
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
          vec2 uv = vUv;

          // 🎯 [Standard Correction] Fragment Shader도 동일하게 뒤집어서 읽음
          vec2 readUv = vec2(vUv.x, 1.0 - vUv.y);

          // 🎯 [핵심 5] GLSL 3.0 샘플링 함수 변경 (texture2D -> texture)
          float tBase = floor(texture(uBase, readUv).r * 255.0 + 0.5);
          float tWater = floor(texture(uWater, readUv).r * 255.0 + 0.5);
          float tTrans = floor(texture(uTransport, readUv).r * 255.0 + 0.5);
          float tStruct = floor(texture(uStructure, readUv).r * 255.0 + 0.5);

          vec3 finalColor = getTypeColor(tBase);
          if (tWater > 0.1) finalColor = getTypeColor(tWater); // TYPE_UNKNOWN 대신 0.1로 안전하게 비교
          if (tTrans > 0.1) finalColor = getTypeColor(tTrans);
          if (tStruct > 0.1) finalColor = getTypeColor(tStruct);

          outColor = vec4(finalColor, 1.0);
        }
      `,
        transparent: false,
        depthWrite: true,
        depthTest: true,
        side: THREE.DoubleSide,
      });
    }, [texBase, texWater, texTransport, texStructure, texHeight]);

    return (
      <mesh
        name={`tile-${key}`}
        position={toThreePosWithOffset(relativePosition, -0.01)}
        rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
        receiveShadow
      >
        <planeGeometry args={[TILE_SIZE, TILE_SIZE, 32, 32]} />
        <primitive object={material} attach="material" />
      </mesh>
    );
  },
);
