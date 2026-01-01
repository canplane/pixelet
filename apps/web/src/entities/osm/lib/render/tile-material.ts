/**
 * @file tile-material.ts
 * 타일 렌더링을 위한 커스텀 ShaderMaterial을 정의합니다.
 * 백엔드 베이커가 생성한 채널들과 Voxel 정의를 SSOT(Single Source of Truth) 기반으로
 * 동적으로 쉐이더를 생성하고 합성하여 최종 색상을 결정합니다.
 *
 * * [Refactor Note]
 * - Major(Channel) / Minor(Type) 2단계 위계 구조를 지원하도록 쉐이더 생성 로직을 고도화했습니다.
 * - 각 채널별로 독립된 컬러 매핑 함수(get[Channel]Color)를 생성합니다.
 */

import * as THREE from 'three';
import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import {
  TILE_CHANNEL_KEYS,
  type TileChannels,
} from '@pixelet/shared/entities/osm/model/tiles';

const { CELL_SIZE, Z_SCALE_RATIO } = OSM_CONSTANTS;

// ----------------------------------------------------------------------
// 1. Helpers for Dynamic GLSL
// ----------------------------------------------------------------------

/** base -> uBase */
const toUniform = (key: string) =>
  `u${key.charAt(0).toUpperCase() + key.slice(1)}`;

/** base -> tBase */
const toVar = (key: string) => `t${key.charAt(0).toUpperCase() + key.slice(1)}`;

/** base -> Base (PascalCase) */
const toChannelName = (key: string) =>
  key.charAt(0).toUpperCase() + key.slice(1).toLowerCase();

/**
 * [Color Uniform Name Generator]
 * Channel + Type Key 조합으로 유니크한 유니폼 이름을 생성합니다.
 * 예: uColorBaseGreenDeep
 */
const toColorUniformName = (channelKey: string, typeKey: string) => {
  const channelPart = toChannelName(channelKey);
  const typePart = typeKey
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join('');
  return `uColor${channelPart}${typePart}`;
};

// ----------------------------------------------------------------------
// 2. Static Parts & Dynamic Uniforms Preparation
// ----------------------------------------------------------------------

const vertexShader = `
  out vec2 vUv;
  uniform usampler2D uHeight; 
  uniform float uVoxelZUnit;

  void main() {
    vUv = uv;
    
    // R16UI 텍스처에서 높이 인덱스 추출 (0~65535)
    // 🎯 [Engineering Fix] Y-Flip 제거: uv를 그대로 사용하여 고도 샘플링
    uint hIndex = texture(uHeight, uv).r; 
    
    // 실제 미터 단위 높이로 변환
    float height = float(hIndex) * uVoxelZUnit;
    
    vec3 pos = position;
    // World Y(North) 방향으로 고도 Displacement 적용
    pos.z += height;

    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
  }
`;

// ----------------------------------------------------------------------
// 3. Dynamic Fragment Shader Generator
// ----------------------------------------------------------------------

const generateFragmentShader = () => {
  // ---------------------------------------------------------
  // [Section A] Channel Related (Pass 0~4)
  // ---------------------------------------------------------

  // 1. Channel Uniforms Auto-Generation
  const channelUniformsDecl = TILE_CHANNEL_KEYS.map(
    (key) => `uniform sampler2D ${toUniform(key)};`,
  ).join('\n  ');

  // 2. Channel Sampling Logic
  // R8 포맷(0~255)을 float로 정확히 복원
  const channelSamplingLogic = TILE_CHANNEL_KEYS.map(
    (key) =>
      `float ${toVar(key)} = floor(texture(${toUniform(key)}, vUv).r * 255.0 + 0.5);`,
  ).join('\n    ');

  // ---------------------------------------------------------
  // [Section B] Voxel Color & Mapping Logic (Major -> Minor)
  // ---------------------------------------------------------

  let colorUniformsDecl = '';
  let colorMappingFunctions = '';

  // 각 채널(Major)별로 독립된 컬러 매핑 함수를 생성합니다.
  (Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]).forEach(
    (majorKey) => {
      // COMMON은 스킵 (실제 렌더링되는 채널이 아님)
      if (majorKey === 'COMMON') return;

      const channelKey = majorKey.toLowerCase();
      const channelTypes = VOXEL_TYPES[majorKey] as Record<string, number>;

      // 1. Color Uniforms Declaration
      const uniforms = Object.keys(channelTypes)
        .filter((key) => key !== 'AIR' && key !== 'UNKNOWN') // AIR/UNKNOWN은 유니폼 불필요
        .map(
          (typeKey) =>
            `uniform vec3 ${toColorUniformName(channelKey, typeKey)};`,
        )
        .join('\n  ');

      colorUniformsDecl += uniforms + '\n  ';

      // 2. Channel-Specific Color Mapping Function
      // vec3 getBaseColor(float type) { ... }
      const cases = Object.keys(channelTypes)
        .filter((key) => key !== 'AIR' && key !== 'UNKNOWN')
        .map((typeKey) => {
          const typeVal = channelTypes[typeKey]; // 정수값 (예: 10)
          const uName = toColorUniformName(channelKey, typeKey);
          // float 비교 시 오차 방지를 위해 Epsilon을 쓰거나 정수로 비교
          return `if (abs(type - ${typeVal}.0) < 0.1) return ${uName};`;
        })
        .join('\n    ');

      colorMappingFunctions += `
  vec3 get${toChannelName(channelKey)}Color(float type) {
    ${cases}
    return vec3(0.0); // Fallback (Black or Transparent logic)
  }
  `;
    },
  );

  // ---------------------------------------------------------
  // [Section C] Channel Blending Logic (Painter's Algorithm)
  // ---------------------------------------------------------

  // UNKNOWN(1) 값
  const unknownVal = `${VOXEL_TYPES.COMMON.UNKNOWN}.0`;
  const baseLandUniform = toColorUniformName('base', 'BASE_LAND'); // Fallback용

  const channelBlendingLogic = TILE_CHANNEL_KEYS.map((key) => {
    const varName = toVar(key);
    const funcName = `get${toChannelName(key)}Color`;

    return `
    // Check [${key}] Channel
    // UNKNOWN(${unknownVal})보다 커야 유효한 데이터
    if (${varName} > ${unknownVal} + 0.1) {
      vec3 channelColor = ${funcName}(${varName});
      // 0.0(Black)이 아니면 유효한 색상으로 간주하여 덮어쓰기
      // (혹은 알파 블렌딩 로직을 추가할 수도 있음)
      finalColor = channelColor;
    }
    `;
  }).join('');

  // ---------------------------------------------------------
  // [Section D] Assembly
  // ---------------------------------------------------------

  return `
  precision highp float;
  precision highp usampler2D;

  // [Auto-Generated Channel Uniforms]
  ${channelUniformsDecl}

  // [Auto-Generated Color Uniforms]
  ${colorUniformsDecl}

  in vec2 vUv;
  out vec4 outColor;

  // [Auto-Generated Color Mapping Functions]
  ${colorMappingFunctions}

  void main() {
    // [Auto-Generated Sampling]
    ${channelSamplingLogic}

    // [Blending Strategy]
    // 0. 캔버스 초기화: 기본 지면 색상 (Base Land) Fallback
    // 만약 모든 채널이 비어있을 경우를 대비해 Base Land 색상으로 초기화합니다.
    vec3 finalColor = ${baseLandUniform};

    // [Auto-Generated Channeling]
    // TILE_CHANNEL_KEYS 순서대로 덮어씌웁니다. (Z-Index: Pass 0 -> Pass N)
    ${channelBlendingLogic}

    outColor = vec4(finalColor, 1.0);
  }
  `;
};

// ----------------------------------------------------------------------
// 4. Factory Function
// ----------------------------------------------------------------------

interface TileMaterialProps {
  channelTextures: Record<keyof TileChannels, THREE.Texture>;
  terrainTexture: THREE.Texture;
}

/**
 * [createTileMaterial]
 * 4개의 채널 텍스처와 1개의 고도 텍스처를 받아 ShaderMaterial을 생성합니다.
 * 🚀 SSOT(TILE_CHANNEL_KEYS + VOXEL_TYPES)에 따라 쉐이더 코드를 완전 동적으로 생성합니다.
 */
export const createTileMaterial = (props: TileMaterialProps) => {
  const { channelTextures, terrainTexture } = props;

  // 1. Voxel Color Uniforms 자동 생성
  const generatedColorUniforms: Record<string, any> = {};

  (Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]).forEach(
    (majorKey) => {
      if (majorKey === 'COMMON') return;

      const channelKey = majorKey.toLowerCase();
      const types = VOXEL_TYPES[majorKey] as Record<string, number>;
      const colors = VOXEL_COLORS[
        majorKey as keyof typeof VOXEL_COLORS
      ] as Record<number, string>;

      Object.keys(types).forEach((typeKey) => {
        // AIR, UNKNOWN 제외하고 색상이 있는 경우만 유니폼 생성
        if (typeKey === 'AIR' || typeKey === 'UNKNOWN') return;

        const typeId = types[typeKey];
        const hexColor = colors[typeId];

        if (hexColor && hexColor !== 'transparent') {
          const uName = toColorUniformName(channelKey, typeKey);
          generatedColorUniforms[uName] = {
            value: new THREE.Color(hexColor),
          };
        }
      });
    },
  );

  // 2. 전체 Uniform 조립 (Terrain + Z-Unit + Colors)
  const uniforms: Record<string, any> = {
    uHeight: { value: terrainTexture },
    uVoxelZUnit: { value: CELL_SIZE * Z_SCALE_RATIO },
    ...generatedColorUniforms,
  };

  // 3. 채널 텍스처 Uniform 자동 주입 (Water, Base...)
  TILE_CHANNEL_KEYS.forEach((key) => {
    if (channelTextures[key]) {
      uniforms[toUniform(key)] = { value: channelTextures[key] };
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
