import React, { useMemo, useEffect } from 'react';
import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { useRelativePosition } from '../../lib/use-coordinate-system';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/osm-tile';
import { THREE_CONFIG } from '../../config/three-config';
import { toThreePosWithOffset } from '@/shared/lib/three/three-coords';

const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO } = OSM_CONSTANTS;

// 🎯 [Code-Cleanliness] 정적 컬러 유니폼을 외부로 분리하여 컴포넌트 내부를 경량화합니다.
const COLOR_UNIFORMS = {
  uColorUnknown: { value: new THREE.Color(VOXEL_COLORS.UNKNOWN) },
  uColorBaseLand: { value: new THREE.Color(VOXEL_COLORS.BASE_LAND) },
  uColorWater: { value: new THREE.Color(VOXEL_COLORS.WATER) },
  uColorGreenDeep: { value: new THREE.Color(VOXEL_COLORS.GREEN_DEEP) },
  uColorGreenLight: { value: new THREE.Color(VOXEL_COLORS.GREEN_LIGHT) },
  uColorSand: { value: new THREE.Color(VOXEL_COLORS.SAND) },
  uColorResidential: { value: new THREE.Color(VOXEL_COLORS.RESIDENTIAL) },
  uColorCommercial: { value: new THREE.Color(VOXEL_COLORS.COMMERCIAL) },
  uColorIndustrial: { value: new THREE.Color(VOXEL_COLORS.INDUSTRIAL) },
  uColorInstitutional: { value: new THREE.Color(VOXEL_COLORS.INSTITUTIONAL) },
  uColorSports: { value: new THREE.Color(VOXEL_COLORS.SPORTS) },
  uColorRoad: { value: new THREE.Color(VOXEL_COLORS.ROAD) },
  uColorPedestrian: { value: new THREE.Color(VOXEL_COLORS.PEDESTRIAN) },
  uColorRail: { value: new THREE.Color(VOXEL_COLORS.RAIL) },
  uColorParking: { value: new THREE.Color(VOXEL_COLORS.PARKING) },
  uColorBuilding: { value: new THREE.Color(VOXEL_COLORS.BUILDING) },
};

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
    THREE.RedIntegerFormat,
    THREE.UnsignedShortType,
  );
  tex.internalFormat = 'R16UI';
  tex.magFilter = THREE.NearestFilter;
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

    // 🎯 [Memory-Leak-Fix] 타일이 언마운트될 때 텍스처와 재질을 반드시 폐기해야 합니다.
    // Three.js 객체는 GC 대상이 아니므로 수동 dispose가 필수입니다.
    useEffect(() => {
      return () => {
        texBase.dispose();
        texWater.dispose();
        texTransport.dispose();
        texStructure.dispose();
        texHeight.dispose();
      };
    }, [texBase, texWater, texTransport, texStructure, texHeight]);

    const material = useMemo(() => {
      const mat = new THREE.ShaderMaterial({
        glslVersion: THREE.GLSL3,
        uniforms: {
          uBase: { value: texBase },
          uWater: { value: texWater },
          uTransport: { value: texTransport },
          uStructure: { value: texStructure },
          uHeight: { value: texHeight },
          uVoxelZUnit: { value: CELL_SIZE * Z_SCALE_RATIO },
          // 🎯 [Code-Cleanliness] Spread 연산자로 유니폼 주입을 간소화
          ...COLOR_UNIFORMS,
        },
        vertexShader: `
        out vec2 vUv;
        uniform usampler2D uHeight; 
        uniform float uVoxelZUnit;

        void main() {
          vUv = uv;
          vec2 readUv = vec2(uv.x, 1.0 - uv.y);
          uint hIndex = texture(uHeight, readUv).r; 
          float height = float(hIndex) * uVoxelZUnit;
          
          vec3 pos = position;
          pos.z += height;

          gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
        }
      `,
        fragmentShader: `
        precision highp float;
        precision highp usampler2D;

        uniform sampler2D uBase;
        uniform sampler2D uWater;
        uniform sampler2D uTransport;
        uniform sampler2D uStructure;

        // Uniform Colors (매크로 확장 대신 명시적 선언 유지)
        uniform vec3 uColorUnknown, uColorBaseLand, uColorWater;
        uniform vec3 uColorGreenDeep, uColorGreenLight, uColorSand;
        uniform vec3 uColorResidential, uColorCommercial, uColorIndustrial, uColorInstitutional, uColorSports;
        uniform vec3 uColorRoad, uColorPedestrian, uColorRail, uColorParking;
        uniform vec3 uColorBuilding;

        in vec2 vUv;
        out vec4 outColor;

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
          if (type == TYPE_WATER) return uColorWater;
          if (type == TYPE_GREEN_DEEP) return uColorGreenDeep;
          if (type == TYPE_GREEN_LIGHT) return uColorGreenLight;
          if (type == TYPE_SAND) return uColorSand;
          if (type == TYPE_RESIDENTIAL) return uColorResidential;
          if (type == TYPE_COMMERCIAL) return uColorCommercial;
          if (type == TYPE_INDUSTRIAL) return uColorIndustrial;
          if (type == TYPE_INSTITUTIONAL) return uColorInstitutional;
          if (type == TYPE_SPORTS) return uColorSports;
          if (type == TYPE_ROAD) return uColorRoad;
          if (type == TYPE_PEDESTRIAN) return uColorPedestrian;          
          if (type == TYPE_RAIL) return uColorRail;
          if (type == TYPE_PARKING) return uColorParking;
          if (type == TYPE_BUILDING) return uColorBuilding;
          return uColorBaseLand;
        }

        void main() {
          vec2 readUv = vec2(vUv.x, 1.0 - vUv.y);

          float tBase = floor(texture(uBase, readUv).r * 255.0 + 0.5);
          float tWater = floor(texture(uWater, readUv).r * 255.0 + 0.5);
          float tTrans = floor(texture(uTransport, readUv).r * 255.0 + 0.5);
          float tStruct = floor(texture(uStructure, readUv).r * 255.0 + 0.5);

          vec3 finalColor = getTypeColor(tBase);
          if (tWater > 0.1) finalColor = getTypeColor(tWater);
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

      return mat;
    }, [texBase, texWater, texTransport, texStructure, texHeight]);

    // 🎯 [Material-Dispose] Material도 폐기 대상입니다.
    useEffect(() => {
      return () => {
        material.dispose();
      };
    }, [material]);

    return (
      <mesh
        name={`tile-${key}`}
        position={toThreePosWithOffset(relativePosition, -0.01)}
        rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
        receiveShadow
      >
        {/* 🎯 [User-Requirement] 해상도는 기존 32x32 유지 */}
        <planeGeometry args={[TILE_SIZE, TILE_SIZE, 32, 32]} />
        <primitive object={material} attach="material" />
      </mesh>
    );
  },
);
