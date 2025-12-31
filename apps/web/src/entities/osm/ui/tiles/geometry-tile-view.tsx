import React, { useMemo, useEffect } from 'react';
import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { useRelativePosition } from '../../lib/use-coordinate-system';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';
import { THREE_CONFIG } from '../../config/three-config';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import {
  getTileBbox,
  getTileKey,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

const {
  TILE_SIZE,
  CELL_SIZE,
  TILE_RESOLUTION: resolution,
  Z_SCALE_RATIO,
} = OSM_CONSTANTS;

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

const createDataTexture = (data: Uint8Array) => {
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

const createHeightTexture = (data: Uint16Array) => {
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
    const { coord, grids } = tile;
    const { layers, terrain } = grids;

    const absoluteCenter = useMemo(() => {
      const { minX, minY } = getTileBbox(coord);
      return {
        x: minX + TILE_SIZE / 2,
        y: minY + TILE_SIZE / 2,
        z: 0,
      };
    }, [coord]);

    // React Cycle에 묶여 있음 (origin 변경 시 재계산됨)
    const relativePosition = useRelativePosition(absoluteCenter);

    const texBase = useMemo(
      () => createDataTexture(layers.base),
      [layers.base],
    );
    const texWater = useMemo(
      () => createDataTexture(layers.water),
      [layers.water],
    );
    const texTransport = useMemo(
      () => createDataTexture(layers.transport),
      [layers.transport],
    );
    const texStructure = useMemo(
      () => createDataTexture(layers.structure),
      [layers.structure],
    );
    const texHeight = useMemo(
      () => createHeightTexture(terrain.height),
      [terrain.height],
    );

    // [Cleanup]
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

        // Uniform Colors
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

    useEffect(() => {
      return () => {
        material.dispose();
      };
    }, [material]);

    return (
      <mesh
        name={`tile-${getTileKey(coord)}`}
        position={toThreeCoordWithOffset(relativePosition, -0.01)}
        rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
        receiveShadow
      >
        <planeGeometry args={[TILE_SIZE, TILE_SIZE, 32, 32]} />
        <primitive object={material} attach="material" />
      </mesh>
    );
  },
);
