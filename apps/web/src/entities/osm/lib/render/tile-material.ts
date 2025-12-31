/**
 * @file tile-material.ts
 * 타일 렌더링을 위한 커스텀 ShaderMaterial을 정의합니다.
 * 백엔드 베이커가 생성한 레이어들과 Voxel 정의를 SSOT(Single Source of Truth) 기반으로
 * 동적으로 쉐이더를 생성하고 합성하여 최종 색상을 결정합니다.
 */

import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import {
  GRID_LAYER_KEYS,
  type GridLayers,
} from '@pixelet/shared/entities/osm/model/tiles';

const { CELL_SIZE, Z_SCALE_RATIO } = OSM_CONSTANTS;

// ----------------------------------------------------------------------
// 1. Helpers for Dynamic GLSL
// ----------------------------------------------------------------------

/** sea -> uSea */
const toUniform = (key: string) =>
  `u${key.charAt(0).toUpperCase() + key.slice(1)}`;

/** sea -> tSea */
const toVar = (key: string) => `t${key.charAt(0).toUpperCase() + key.slice(1)}`;

/** GREEN_DEEP -> GreenDeep (For uniform names) */
const toPascalCase = (str: string) =>
  str
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join('');

// 🎯 Voxel SSOT 추출 (AIR 제외: 렌더링되지 않음)
const VOXEL_KEYS = (
  Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]
).filter((key) => key !== 'AIR');

// ----------------------------------------------------------------------
// 2. Static Parts & Dynamic Uniforms Preparation
// ----------------------------------------------------------------------

const vertexShader = `
  out vec2 vUv;
  uniform usampler2D uHeight; 
  uniform float uVoxelZUnit;

  void main() {
    vUv = uv;
    // 텍스처 좌표계 보정 (Y-Flip for Three.js PlaneGeometry)
    vec2 readUv = vec2(uv.x, 1.0 - uv.y);
    
    // R16UI 텍스처에서 높이 인덱스 추출 (0~65535)
    uint hIndex = texture(uHeight, readUv).r; 
    
    // 실제 미터 단위 높이로 변환
    float height = float(hIndex) * uVoxelZUnit;
    
    vec3 pos = position;
    pos.z += height;

    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
  }
`;

// ----------------------------------------------------------------------
// 3. Dynamic Fragment Shader Generator
// ----------------------------------------------------------------------

const generateFragmentShader = () => {
  // ---------------------------------------------------------
  // [Section A] Layer Related (Pass 0~4)
  // ---------------------------------------------------------

  // 1. Layer Uniforms Auto-Generation
  // Output: uniform sampler2D uSea; uniform sampler2D uBase; ...
  const layerUniformsDecl = GRID_LAYER_KEYS.map(
    (key) => `uniform sampler2D ${toUniform(key)};`,
  ).join('\n  ');

  // 2. Layer Sampling Logic Auto-Generation
  // Output: float tSea = floor(texture(uSea, readUv).r * 255.0 + 0.5);
  const layerSamplingLogic = GRID_LAYER_KEYS.map(
    (key) =>
      `float ${toVar(key)} = floor(texture(${toUniform(key)}, readUv).r * 255.0 + 0.5);`,
  ).join('\n    ');

  // 3. Layer Blending Logic Auto-Generation (Painter's Algorithm)
  // GRID_LAYER_KEYS 순서(Sea -> Base -> ... -> Structure)가 곧 Z-Index 역할을 합니다.
  const layerBlendingLogic = GRID_LAYER_KEYS.map(
    (key) => `
    // Check [${key}] Layer
    if (${toVar(key)} > 0.1) {
      finalColor = getTypeColor(${toVar(key)});
    }`,
  ).join('');

  // ---------------------------------------------------------
  // [Section B] Voxel Color Related (From voxels.ts)
  // ---------------------------------------------------------

  // 1. Color Uniforms Declaration
  // Output: uniform vec3 uColorGreenDeep; ...
  const colorUniformsDecl = VOXEL_KEYS.map(
    (key) => `uniform vec3 uColor${toPascalCase(key)};`,
  ).join('\n  ');

  // 2. Voxel Type Macros
  // Output: #define TYPE_GREEN_DEEP 10.0
  const voxelMacrosDecl = VOXEL_KEYS.map(
    (key) => `#define TYPE_${key} ${VOXEL_TYPES[key]}.0`,
  ).join('\n  ');

  // 3. getTypeColor Logic Auto-Generation
  // Output: if (type == TYPE_GREEN_DEEP) return uColorGreenDeep;
  const colorMappingLogic = VOXEL_KEYS.map(
    (key) => `if (type == TYPE_${key}) return uColor${toPascalCase(key)};`,
  ).join('\n    ');

  // ---------------------------------------------------------
  // [Section C] Assembly
  // ---------------------------------------------------------

  return `
  precision highp float;
  precision highp usampler2D;

  // [Auto-Generated Layer Uniforms]
  ${layerUniformsDecl}

  // [Auto-Generated Color Uniforms]
  ${colorUniformsDecl}

  in vec2 vUv;
  out vec4 outColor;

  // [Auto-Generated Voxel Macros]
  ${voxelMacrosDecl}

  vec3 getTypeColor(float type) {
    // [Auto-Generated Color Mapping]
    ${colorMappingLogic}
    
    // Default Fallback
    return uColorBaseLand;
  }

  void main() {
    vec2 readUv = vec2(vUv.x, 1.0 - vUv.y);

    // [Auto-Generated Sampling]
    ${layerSamplingLogic}

    // [Blending Strategy]
    // 0. 캔버스 초기화: 기본 지면 색상 (Base Land)
    vec3 finalColor = uColorBaseLand;

    // [Auto-Generated Layering]
    // GRID_LAYER_KEYS 순서대로 덮어씌웁니다. (Z-Index: Pass 0 -> Pass N)
    ${layerBlendingLogic}

    outColor = vec4(finalColor, 1.0);
  }
  `;
};

// ----------------------------------------------------------------------
// 4. Factory Function
// ----------------------------------------------------------------------

interface TileMaterialProps {
  layerTextures: Record<keyof GridLayers, THREE.Texture>;
  terrainTexture: THREE.Texture;
}

/**
 * [createTileMaterial]
 * 5개의 레이어 텍스처와 1개의 고도 텍스처를 받아 ShaderMaterial을 생성합니다.
 * 🚀 SSOT(GRID_LAYER_KEYS + VOXEL_TYPES)에 따라 쉐이더 코드를 완전 동적으로 생성합니다.
 */
export const createTileMaterial = (props: TileMaterialProps) => {
  const { layerTextures, terrainTexture } = props;

  // 1. Voxel Color Uniforms 자동 생성
  const generatedColorUniforms: Record<string, any> = {};
  VOXEL_KEYS.forEach((key) => {
    // VOXEL_COLORS에 정의된 색상 문자열을 THREE.Color로 변환하여 주입
    // (Partial 타입 방어를 위해 key 체크)
    const colorHex = VOXEL_COLORS[key as keyof typeof VOXEL_COLORS];
    if (colorHex) {
      generatedColorUniforms[`uColor${toPascalCase(key)}`] = {
        value: new THREE.Color(colorHex),
      };
    }
  });

  // 2. 전체 Uniform 조립 (Terrain + Z-Unit + Colors)
  const uniforms: Record<string, any> = {
    uHeight: { value: terrainTexture },
    uVoxelZUnit: { value: CELL_SIZE * Z_SCALE_RATIO },
    ...generatedColorUniforms,
  };

  // 3. 레이어 텍스처 Uniform 자동 주입 (Sea, Base, Water...)
  GRID_LAYER_KEYS.forEach((key) => {
    if (layerTextures[key]) {
      uniforms[toUniform(key)] = { value: layerTextures[key] };
    }
  });

  return new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    vertexShader,
    // 🚀 함수 실행 결과(Generated String)를 쉐이더 소스로 주입
    fragmentShader: generateFragmentShader(),
    transparent: false,
    depthWrite: true,
    depthTest: true,
    side: THREE.DoubleSide,
  });
};
