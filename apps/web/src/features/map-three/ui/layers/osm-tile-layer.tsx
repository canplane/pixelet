import React, { useMemo } from 'react';
import * as THREE from 'three';
import { VOXEL_TYPES } from '@/entities/osm/lib/tile-baker';
import { OSM_CONFIG } from '@/entities/osm/config/osm-config';
import { VOXEL_COLORS } from '@/entities/osm/config/map-theme';
import { useRelativePosition } from '../../model/use-relative-position';
import type { WorldTile } from '../../model/map-store';
import { THREE_CONFIG } from '../../model/three-config';

interface OsmTileLayerProps {
  tile: WorldTile;
}

/**
 * [osm-tile-layer]
 * 타일 기반 복셀 데이터를 GPU DataTexture로 변환하여 쉐이더로 렌더링합니다.
 * VOXEL_COLORS 테마와 VOXEL_TYPES 상수를 엄격하게 참조하여 지형을 시각화합니다.
 */
export const OsmTileLayer = React.memo(({ tile }: OsmTileLayerProps) => {
  const { grid, origin, resolution, key } = tile;
  const TILE_SIZE = OSM_CONFIG.TILE_SIZE;

  /**
   * [calculate-absolute-center]
   * PlaneGeometry의 피벗이 중앙이므로 타일의 월드 좌표 중심점을 계산합니다.
   */
  const absoluteCenter = useMemo(
    () => ({
      x: origin.x + TILE_SIZE / 2,
      y: 0,
      z: origin.z + TILE_SIZE / 2,
    }),
    [origin.x, origin.z, TILE_SIZE],
  );

  const relativePosition = useRelativePosition(absoluteCenter);

  /**
   * [generate-gpu-texture]
   * 그리드 데이터를 R8(Red 8bit) 포맷 텍스처로 변환합니다.
   * NearestFilter를 사용하여 복셀 경계면의 안티앨리어싱 없는 격자 형태를 유지합니다.
   */
  const texture = useMemo(() => {
    const tex = new THREE.DataTexture(
      grid,
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
  }, [grid, resolution]);

  /**
   * [terrain-shader-material]
   * VOXEL_COLORS 테마의 모든 색상을 유니폼으로 주입하고,
   * 프래그먼트 쉐이더에서 타입 인덱스(0-255)에 따라 정밀하게 분기 렌더링합니다.
   */
  const material = useMemo(() => {
    return new THREE.ShaderMaterial({
      uniforms: {
        uMap: { value: texture },
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
        uColorEmpty: { value: new THREE.Color(VOXEL_COLORS.EMPTY) },
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
        uniform sampler2D uMap;
        uniform vec3 uColorWater, uColorRoad, uColorBuilding, uColorGreenDeep, uColorGreenLight;
        uniform vec3 uColorBaseLand, uColorResidential, uColorCommercial, uColorIndustrial;
        uniform vec3 uColorSand, uColorRail, uColorPedestrian, uColorEmpty;
        varying vec2 vUv;

        // [voxel-type-mapping]
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

        void main() {
          /**
           * [uv-coordinate-correction]
           * 데이터 그리드와 월드 좌표계의 북쪽 방향을 일치시키기 위해 V(y) 축을 반전합니다.
           */
          vec2 correctedUv = vec2(vUv.x, 1.0 - vUv.y);
          
          /**
           * [type-extraction]
           * DataTexture에서 인덱스 값을 추출합니다. 
           * 부동소수점 오차 보정을 위해 0.5 offset 후 floor를 적용합니다.
           */
          float type = floor(texture2D(uMap, correctedUv).r * 255.0 + 0.5);
          
          // 기본 색상은 BASE_LAND로 설정
          vec3 color = uColorBaseLand;

          if (abs(type - TYPE_EMPTY) < 0.1) color = uColorEmpty;
          else if (abs(type - TYPE_WATER) < 0.1) color = uColorWater;
          else if (abs(type - TYPE_ROAD) < 0.1) color = uColorRoad;
          else if (abs(type - TYPE_BUILDING) < 0.1) color = uColorBuilding;
          else if (abs(type - TYPE_GREEN_DEEP) < 0.1) color = uColorGreenDeep;
          else if (abs(type - TYPE_GREEN_LIGHT) < 0.1) color = uColorGreenLight;
          else if (abs(type - TYPE_BASE_LAND) < 0.1) color = uColorBaseLand;
          else if (abs(type - TYPE_RESIDENTIAL) < 0.1) color = uColorResidential;
          else if (abs(type - TYPE_COMMERCIAL) < 0.1) color = uColorCommercial;
          else if (abs(type - TYPE_INDUSTRIAL) < 0.1) color = uColorIndustrial;
          else if (abs(type - TYPE_SAND) < 0.1) color = uColorSand;
          else if (abs(type - TYPE_PEDESTRIAN) < 0.1) color = uColorPedestrian;
          else if (abs(type - TYPE_RAIL) < 0.1) color = uColorRail;
          
          gl_FragColor = vec4(color, 1.0);
        }
      `,
      transparent: false,
      side: THREE.FrontSide,
    });
  }, [texture]);

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
