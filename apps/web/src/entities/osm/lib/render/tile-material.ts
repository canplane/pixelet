/**
 * @file tile-material.ts
 * 타일 렌더링을 위한 커스텀 ShaderMaterial을 정의합니다.
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

const { CELL_SIZE } = OSM_CONSTANTS;

// ----------------------------------------------------------------------
// 1. Helpers for Dynamic GLSL
// ----------------------------------------------------------------------

const toUniform = (key: string) =>
  `u${key.charAt(0).toUpperCase() + key.slice(1)}`;

const toVar = (key: string) => `t${key.charAt(0).toUpperCase() + key.slice(1)}`;

const toChannelName = (key: string) =>
  key.charAt(0).toUpperCase() + key.slice(1).toLowerCase();

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
  precision highp float;
  precision highp usampler2D;

  // 1. Fog 파라미터 선언 (vFogDepth 등)
  #include <fog_pars_vertex>

  out vec2 vUv;
  out vec2 vBoxUv; // 🚀 Box UV 확실하게 출력
  out float vIsSideFace;
  
  uniform usampler2D uHeight; 
  uniform float uVoxelZUnit;
  uniform vec2 uResolution; 
  uniform float uVoxelSize;
  uniform float uLodScale;

  void main() {
    vec2 lodRes = uResolution / uLodScale;
    int iLodResX = int(lodRes.x);
    int iLodResY = int(lodRes.y);
    
    int ixLod = gl_InstanceID % iLodResX;
    int iyLod = gl_InstanceID / iLodResX;
    
    int ix = ixLod * int(uLodScale);
    int iy = iyLod * int(uLodScale);

    // 🎯 [Safety] 픽셀 중앙 샘플링
    vec2 pixelPos = vec2(float(ix), float(iy));
    vUv = (pixelPos + 0.5) / uResolution;

    // 🚀 [Verify] BoxGeometry의 UV를 그대로 캡처
    vBoxUv = uv;

    uint h = texelFetch(uHeight, ivec2(ix, iy), 0).r;
    float zSurface = float(h) * uVoxelZUnit; 
    
    float visualVoxelSize = uVoxelSize * uLodScale;
    float zCenter = zSurface - (visualVoxelSize * 0.5);
    
    vec2 offset = lodRes * 0.5;
    vec3 instancePos = vec3(
        (float(ixLod) - offset.x + 0.5) * visualVoxelSize,
        (float(iyLod) - offset.y + 0.5) * visualVoxelSize,
        zCenter
    );
    
    vec3 localPos = position;

    // --- Gap Filling & Skirt Logic ---
    int iResX = int(uResolution.x);
    int iResY = int(uResolution.y);
    int lodStep = int(uLodScale);

    // 🎯 [Skirt Fix] 현재 복셀이 타일 가장자리인지 확인
    bool isEdgeX = (ixLod == 0 || ixLod == iLodResX - 1);
    bool isEdgeY = (iyLod == 0 || iyLod == iLodResY - 1);
    
    float targetBottom;

    if (isEdgeX || isEdgeY) {
        // 🚀 가장자리는 이웃 높이를 모르므로 바닥을 지하 깊이 내림 (Skirt)
        targetBottom = zSurface - 200.0;
    } else {
        // 내부는 기존대로 인접 4방향 중 가장 낮은 높이까지 내림
        uint hN = texelFetch(uHeight, ivec2(ix, clamp(iy + lodStep, 0, iResY - 1)), 0).r;
        uint hS = texelFetch(uHeight, ivec2(ix, clamp(iy - lodStep, 0, iResY - 1)), 0).r;
        uint hE = texelFetch(uHeight, ivec2(clamp(ix + lodStep, 0, iResX - 1), iy), 0).r;
        uint hW = texelFetch(uHeight, ivec2(clamp(ix - lodStep, 0, iResX - 1), iy), 0).r;
        
        float minNeighborSurface = float(min(min(hN, hS), min(hE, hW))) * uVoxelZUnit;
        targetBottom = minNeighborSurface - visualVoxelSize;
    }

    float myBottom = zSurface - visualVoxelSize;

    // 복셀 바닥 연장 (내 바닥보다 목표 바닥이 낮으면 연장)
    if (localPos.z < 0.0 && targetBottom < myBottom) {
       float extraDrop = (myBottom - targetBottom) / visualVoxelSize;
       localPos.z -= extraDrop;
    }
    
    vIsSideFace = (position.z < 0.45) ? 1.0 : 0.0;

    // 1.002배 미세 확대 (틈새 메우기)
    vec3 finalSize = localPos * visualVoxelSize * 1.002;
    vec3 transformed = finalSize + instancePos;

    // 🚀 [Fix 1] mvPosition 명시적 계산
    // Fog 청크는 'mvPosition'이라는 변수명을 필요로 합니다.
    vec4 worldPosition = modelMatrix * vec4(transformed, 1.0);
    vec4 mvPosition = viewMatrix * worldPosition;
    
    gl_Position = projectionMatrix * mvPosition;

    // Fog 깊이 계산 (mvPosition 필요)
    #include <fog_vertex>
  }
`;

const generateFragmentShader = () => {
  const channelUniformsDecl = [
    ...TILE_CHANNEL_KEYS.map((key) => `uniform sampler2D ${toUniform(key)};`),
    `uniform sampler2D uEditMask;`,
  ].join('\n  ');

  const channelSamplingLogic = TILE_CHANNEL_KEYS.map(
    (key) =>
      `float ${toVar(key)} = floor(texture(${toUniform(key)}, vUv).r * 255.0 + 0.5);`,
  ).join('\n    ');

  let colorUniformsDecl = '';
  let colorMappingFunctions = '';

  (Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]).forEach(
    (majorKey) => {
      if (majorKey === 'COMMON') return;

      const channelKey = majorKey.toLowerCase();
      const channelTypes = VOXEL_TYPES[majorKey] as Record<string, number>;

      const uniforms = Object.keys(channelTypes)
        .filter((key) => key !== 'AIR' && key !== 'UNKNOWN')
        .map(
          (typeKey) =>
            `uniform vec3 ${toColorUniformName(channelKey, typeKey)};`,
        )
        .join('\n  ');
      colorUniformsDecl += uniforms + '\n  ';

      const cases = Object.keys(channelTypes)
        .filter((key) => key !== 'AIR' && key !== 'UNKNOWN')
        .map((typeKey) => {
          const typeVal = channelTypes[typeKey];
          const uName = toColorUniformName(channelKey, typeKey);
          return `if (abs(type - ${typeVal}.0) < 0.1) return ${uName};`;
        })
        .join('\n    ');

      colorMappingFunctions += `
      vec3 get${toChannelName(channelKey)}Color(float type) {
        ${cases}
        return vec3(0.0);
      }
      `;
    },
  );

  const unknownVal = `${VOXEL_TYPES.COMMON.UNKNOWN}.0`;
  const baseLandUniform = toColorUniformName('base', 'BASE_LAND');

  const channelBlendingLogic = TILE_CHANNEL_KEYS.map((key) => {
    const varName = toVar(key);
    const funcName = `get${toChannelName(key)}Color`;
    return `
    if (${varName} > ${unknownVal} + 0.1) {
      vec3 channelColor = ${funcName}(${varName});
      finalColor = channelColor;
    }
    `;
  }).join('');

  return `
  precision highp float;
  precision highp usampler2D;

  // Fog Uniforms (fogColor, fogDensity 등)
  #include <fog_pars_fragment>

  ${channelUniformsDecl}
  ${colorUniformsDecl}
  
  uniform vec3 uBorderColor;
  uniform float uBorderWidth;
  uniform float uBorderOpacity;

  in vec2 vUv;
  in vec2 vBoxUv; 
  in float vIsSideFace;
  
  out vec4 outColor;

  ${colorMappingFunctions}

  // 🎯 [Grid Function] 앤티앨리어싱 그리드 (Anti-aliased Grid)
  // fwidth를 사용하여 픽셀 단위로 부드러운 선을 그립니다.
  float gridFactor(vec2 uv, float width) {
    // uv의 변화량(미분값)을 구해 픽셀 경계를 찾음
    vec2 grid = abs(fract(uv - 0.5) - 0.5) / fwidth(uv);
    float line = min(grid.x, grid.y);
    return 1.0 - min(line / width, 1.0);
  }

  void main() {
    float editMask = texture(uEditMask, vUv).r;
    if (editMask > 0.5) {
      discard;
    }

    ${channelSamplingLogic}

    vec3 finalColor = ${baseLandUniform};
    ${channelBlendingLogic}

    if (vIsSideFace > 0.5) {
      finalColor *= 0.8; 
    }

    // 🚀 [Improved Grid Logic]
    // step 방식 대신 fwidth 방식을 사용하거나, step을 확실하게 적용합니다.
    // 여기서는 사용자 의도(물리적 두께)를 존중하여 step 방식을 강화합니다.
    
    vec2 thick = vec2(uBorderWidth);
    // 가장자리(0.0, 1.0) 근처인지 판별
    vec2 grid = step(thick, vBoxUv) * step(vBoxUv, vec2(1.0) - thick);
    float isBorder = 1.0 - (grid.x * grid.y); 

    if (isBorder > 0.5) {
        finalColor = mix(finalColor, uBorderColor, uBorderOpacity);
    }

    outColor = vec4(finalColor, 1.0);

    // 🚀 [Fix 2] Fog 수동 적용 (GLSL3 호환)
    // <fog_fragment>는 gl_FragColor를 쓰므로 사용하지 않고 직접 계산합니다.
    #ifdef USE_FOG
      float fogFactor = 0.0;
      #ifdef FOG_EXP2
        fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
      #else
        fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
      #endif
      
      outColor.rgb = mix( outColor.rgb, fogColor, fogFactor );
    #endif
  }
  `;
};

interface TileMaterialProps {
  channelTextures: Record<keyof TileChannels, THREE.Texture>;
  terrainTexture: THREE.Texture;
  uResolution: THREE.Vector2;
  uVoxelSize: number;
  uLodScale: number;
  uEditMask?: THREE.Texture;
  gridConfig?: {
    uBorderColor: THREE.Color;
    uBorderWidth: number;
    uBorderOpacity: number;
  };
}

export const createTileMaterial = (props: TileMaterialProps) => {
  const {
    channelTextures,
    terrainTexture,
    uResolution,
    uVoxelSize,
    uLodScale,
    uEditMask,
    gridConfig,
  } = props;

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

  // 🚀 [Restore] 기본값 설정 및 Active Config 결정
  const defaultGrid = {
    uBorderColor: new THREE.Color('#000000'),
    uBorderWidth: 0.0,
    uBorderOpacity: 0.0,
  };
  const activeGrid = gridConfig || defaultGrid;

  const uniforms: Record<string, any> = {
    ...THREE.UniformsLib.fog,
    uHeight: { value: terrainTexture },
    uVoxelZUnit: { value: CELL_SIZE },
    uResolution: { value: uResolution },
    uVoxelSize: { value: uVoxelSize },
    uLodScale: { value: uLodScale },
    uEditMask: {
      value: uEditMask || new THREE.DataTexture(new Uint8Array([0]), 1, 1),
    },
    // 🚀 [Restore] GPU로 데이터를 보내는 핵심 연결 고리
    uBorderColor: { value: activeGrid.uBorderColor },
    uBorderWidth: { value: activeGrid.uBorderWidth },
    uBorderOpacity: { value: activeGrid.uBorderOpacity },

    ...generatedColorUniforms,
  };

  TILE_CHANNEL_KEYS.forEach((key) => {
    if (channelTextures[key]) {
      uniforms[toUniform(key)] = { value: channelTextures[key] };
    }
  });

  return new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    // generateFragmentShader(), vertexShader 변수는 파일 상단에 정의된 것을 사용
    vertexShader,
    fragmentShader: generateFragmentShader(),
    transparent: false,
    depthWrite: true,
    depthTest: true,
    side: THREE.DoubleSide,
    fog: true,
  });
};
