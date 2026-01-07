/**
 * @file tile-material.ts
 * 타일 렌더링을 위한 커스텀 ShaderMaterial을 정의합니다.
 */

import * as THREE from 'three';
import { BASE_COLORS, BASE_TYPES } from '@pixelet/core/entities/cell/model/base';
import { ColorOps } from '@pixelet/core/entities/cell/model/utils';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { TILE_SURFACE_KEYS, TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';

const { CELL_SIZE } = GEO_CONSTANTS;

// ----------------------------------------------------------------------
// 🎯 [Render Configuration] 렌더링 스타일 결정
// ----------------------------------------------------------------------
const RENDER_CONFIG = {
  // true: 인접 셀과의 고도차를 계산해 부드러운 경사면을 만듭니다. (Smooth)
  // false: 고도차를 무시하고 깍두기 모양의 날카로운 복셀 형태를 유지합니다. (Sharp/Minecraft Style)
  USE_SLOPE_SMOOTHING: false,
};

// ----------------------------------------------------------------------
// 1. Helpers
// ----------------------------------------------------------------------

const toUniform = (key: string) => `u${key.charAt(0).toUpperCase() + key.slice(1)}`;
const toVar = (key: string) => `t${key.charAt(0).toUpperCase() + key.slice(1)}`;
const toChannelName = (key: string) => key.charAt(0).toUpperCase() + key.slice(1).toLowerCase();
const toColorUniformName = (channelKey: string, typeKey: string) => {
  const channelPart = toChannelName(channelKey);
  const typePart = typeKey
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join('');
  return `uColor${channelPart}${typePart}`;
};

// ----------------------------------------------------------------------
// 2. Vertex Shader (Unified Slope Logic)
// ----------------------------------------------------------------------
const vertexShader = `
  precision highp float;
  precision highp isampler2D;
  #include <fog_pars_vertex>

  out vec2 vUv;
  out vec2 vBoxUv; 
  out float vIsSideFace;
  out float vIsSkirt;
  out float vViewDist;
  out vec3 vViewPosition;

  uniform isampler2D uHeight;
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

    vec2 pixelPos = vec2(float(ix), float(iy));
    vUv = (pixelPos + 0.5) / uResolution;
    vBoxUv = uv;

    int h = texelFetch(uHeight, ivec2(ix, iy), 0).r;
    float zSurface = float(h) * uVoxelZUnit; 
    float visualVoxelSize = uVoxelSize * uLodScale;
    float zCenter = zSurface - (visualVoxelSize * 0.5);
    vec2 offset = lodRes * 0.5;
    vec3 instancePos = vec3((float(ixLod) - offset.x + 0.5) * visualVoxelSize, (float(iyLod) - offset.y + 0.5) * visualVoxelSize, zCenter);
    
    vec3 localPos = position;

    int iResX = int(uResolution.x);
    int iResY = int(uResolution.y);
    int lodStep = int(uLodScale);
    bool isEdgeX = (ixLod == 0 || ixLod == iLodResX - 1);
    bool isEdgeY = (iyLod == 0 || iyLod == iLodResY - 1);
    
    // 🚀 [Logic Unification] 
    int yN = clamp(iy + lodStep, 0, iResY - 1);
    int yS = clamp(iy - lodStep, 0, iResY - 1);
    int xE = clamp(ix + lodStep, 0, iResX - 1);
    int xW = clamp(ix - lodStep, 0, iResX - 1);

    int hN  = texelFetch(uHeight, ivec2(ix, yN), 0).r;
    int hS  = texelFetch(uHeight, ivec2(ix, yS), 0).r;
    int hE  = texelFetch(uHeight, ivec2(xE, iy), 0).r;
    int hW  = texelFetch(uHeight, ivec2(xW, iy), 0).r;

    int hNE = texelFetch(uHeight, ivec2(xE, yN), 0).r;
    int hNW = texelFetch(uHeight, ivec2(xW, yN), 0).r;
    int hSE = texelFetch(uHeight, ivec2(xE, yS), 0).r;
    int hSW = texelFetch(uHeight, ivec2(xW, yS), 0).r;

    // 스커트 목표 높이 설정
    float targetBottom;
    if (isEdgeX || isEdgeY) {
        targetBottom = zSurface - 200.0; // 가장자리는 스커트 내리기
    } else {
        targetBottom = float(min(min(hN, hS), min(hE, hW))) * uVoxelZUnit - visualVoxelSize;
    }

    // 🚀 [Slope Calculation] 가장자리도 계산에 참여시킵니다!
    float dropN = 0.0, dropS = 0.0, dropE = 0.0, dropW = 0.0;
    float dropNE = 0.0, dropNW = 0.0, dropSE = 0.0, dropSW = 0.0;

    if (h > hN)  dropN  = min(float(h - hN), 1.0);
    if (h > hS)  dropS  = min(float(h - hS), 1.0);
    if (h > hE)  dropE  = min(float(h - hE), 1.0);
    if (h > hW)  dropW  = min(float(h - hW), 1.0);
    
    if (h > hNE) dropNE = min(float(h - hNE), 1.0);
    if (h > hNW) dropNW = min(float(h - hNW), 1.0);
    if (h > hSE) dropSE = min(float(h - hSE), 1.0);
    if (h > hSW) dropSW = min(float(h - hSW), 1.0);

    float myBottom = zSurface - visualVoxelSize;
    vIsSkirt = 0.0; 
    
    if (localPos.z < 0.0 && targetBottom < myBottom) {
       float extraDrop = (myBottom - targetBottom) / visualVoxelSize;
       localPos.z -= extraDrop;
       if (isEdgeX || isEdgeY) vIsSkirt = 1.0;
    }
    
    vIsSideFace = (position.z < 0.45) ? 1.0 : 0.0;

    // 🎯 [Apply Slope / Voxel Toggle] 
    // RENDER_CONFIG.USE_SLOPE_SMOOTHING 값이 true일 때만 경사면 로직이 작동합니다.
    #if ${RENDER_CONFIG.USE_SLOPE_SMOOTHING ? 1 : 0}
    // 🎯 [Apply Slope] 
    if (localPos.z > 0.25) { 
        bool isNorth = localPos.y > 0.0;
        bool isSouth = localPos.y < 0.0;
        bool isEast  = localPos.x > 0.0;
        bool isWest  = localPos.x < 0.0;

        float maxDrop = 0.0;

        if (isNorth && isEast) maxDrop = max(maxDrop, max(dropN, max(dropE, dropNE)));
        if (isNorth && isWest) maxDrop = max(maxDrop, max(dropN, max(dropW, dropNW)));
        if (isSouth && isEast) maxDrop = max(maxDrop, max(dropS, max(dropE, dropSE)));
        if (isSouth && isWest) maxDrop = max(maxDrop, max(dropS, max(dropW, dropSW)));

        if (isNorth && !isEast && !isWest) maxDrop = max(maxDrop, dropN);
        if (isSouth && !isEast && !isWest) maxDrop = max(maxDrop, dropS);
        if (isEast  && !isNorth && !isSouth) maxDrop = max(maxDrop, dropE);
        if (isWest  && !isNorth && !isSouth) maxDrop = max(maxDrop, dropW);

        localPos.z -= maxDrop; 
    }
    #endif

    vec3 finalSize = localPos * visualVoxelSize * 1.002;
    vec3 transformed = finalSize + instancePos;
    
    vec4 worldPosition = modelMatrix * vec4(transformed, 1.0);
    vec4 mvPosition = viewMatrix * worldPosition;
    
    gl_Position = projectionMatrix * mvPosition;

    vViewPosition = mvPosition.xyz;
    vViewDist = -mvPosition.z;

    #include <fog_vertex>
  }
`;

// ----------------------------------------------------------------------
// 3. Fragment Shader (No Fake Darkening)
// ----------------------------------------------------------------------

const generateFragmentShader = () => {
  const channelUniformsDecl = [...TILE_SURFACE_KEYS.map((key) => `uniform sampler2D ${toUniform(key)};`), `uniform sampler2D uEditMask;`].join('\n  ');
  const channelSamplingLogic = TILE_SURFACE_KEYS.map((key) => `float ${toVar(key)} = floor(texture(${toUniform(key)}, vUv).r * 255.0 + 0.5);`).join('\n    ');

  let colorUniformsDecl = '';
  let colorMappingFunctions = '';

  Object.keys(BASE_TYPES).forEach((majorKey) => {
    const channelKey = majorKey.toLowerCase();
    const types = BASE_TYPES[majorKey];
    const colors = BASE_COLORS[majorKey];

    // 🎯 [Optimization] isValidHexRGB 조건에 맞는 타입만 셰이더 코드 생성에 참여
    const validTypes = Object.keys(types).filter((typeKey) => ColorOps.isValidHex(colors?.[types[typeKey]]));
    if (validTypes.length > 0) {
      colorUniformsDecl += validTypes.map((typeKey) => `uniform vec3 ${toColorUniformName(channelKey, typeKey)};`).join('\n  ') + '\n  ';

      const cases = validTypes.map((typeKey) => `if (abs(type - ${types[typeKey]}.0) < 0.1) return ${toColorUniformName(channelKey, typeKey)};`).join('\n    ');

      colorMappingFunctions += `vec3 get${toChannelName(channelKey)}Color(float type) { ${cases} return vec3(0.0); }\n`;
    } else {
      // 유효한 색상이 없는 채널(예: 전체 AIR)을 위한 Fallback 함수
      colorMappingFunctions += `vec3 get${toChannelName(channelKey)}Color(float type) { return vec3(0.0); }\n`;
    }
  });

  const baseLandUniform = toColorUniformName('land', 'BARE');

  // 🎨 [Painter's Algorithm] 레이어 합성 로직 생성
  const channelBlendingLogic = TILE_SURFACE_KEYS.map((key) => {
    const id = toVar(key);
    const getter = `get${toChannelName(key)}Color`;
    return `if (${id} > 0.5) finalColor = ${getter}(${id});`;
  }).join('');

  return `
  precision highp float;
  #include <fog_pars_fragment>

  ${channelUniformsDecl}
  ${colorUniformsDecl}
  
  uniform vec3 uBorderColor;
  uniform float uBorderOpacity;

  in vec2 vUv;
  in vec2 vBoxUv; 
  in float vIsSideFace;
  in float vIsSkirt;
  in float vViewDist;
  in vec3 vViewPosition;
  
  out vec4 outColor;

  ${colorMappingFunctions}

  float getDynamicGrid(vec2 uv, float targetWidth) {
    vec2 fw = fwidth(uv);
    vec2 dist = abs(fract(uv - 0.5) - 0.5);
    vec2 normDist = dist / fw;
    float lineX = 1.0 - smoothstep(targetWidth - 0.5, targetWidth + 0.5, normDist.x);
    float lineY = 1.0 - smoothstep(targetWidth - 0.5, targetWidth + 0.5, normDist.y);
    return max(lineX, lineY);
  }

  float getLuminance(vec3 color) {
    return dot(color, vec3(0.299, 0.587, 0.114));
  }

  void main() {
    float editMask = texture(uEditMask, vUv).r;
    if (editMask > 0.5) discard;

    ${channelSamplingLogic}
    vec3 finalColor = ${baseLandUniform};
    ${channelBlendingLogic}

    // 👑 [Normal Reconstruction]
    vec3 fdx = dFdx(vViewPosition);
    vec3 fdy = dFdy(vViewPosition);
    vec3 normal = normalize(cross(fdx, fdy));

    // ☀️ [Natural Lighting]
    vec3 lightDir = normalize(vec3(0.5, 0.5, 1.0)); 
    float diff = max(dot(normal, lightDir), 0.0);
    
    // 🚀 [Fix Grid Lines] 
    // 스커트와 옆면을 강제로 어둡게 만들던 코드를 제거했습니다.
    // 이제 오직 '각도'에 따른 자연스러운 음영만 남습니다.
    float lightIntensity = 0.65 + (0.35 * diff); 

    // (선택사항) 아주 미세한 깊이감만 추가 (눈에 띄지 않게)
    if (vIsSideFace > 0.5) lightIntensity *= 0.98;

    finalColor *= lightIntensity;

    // 🕸️ Grid Fix (Anti-Grain)
    if (vIsSkirt < 0.5 && vIsSideFace < 0.5) {
        // 1. 거리 페이드 조정 (0~600m)
        float distFactor = smoothstep(0.0, 600.0, vViewDist);
        float distFade = 1.0 - distFactor;
        
        // 2. 두께 감소 (가까이 1.5px -> 멀리 0.5px)
        // 너무 얇아지면(0.0) 오히려 픽셀이 끊겨 보여서 자글거림. 최소 두께 0.5 유지
        float currentWidth = mix(1.5, 0.5, distFactor);

        // 3. 투명도 가속 감소
        // 거리가 멀어질수록(distFactor 증가) 투명도를 제곱으로 빠르게 떨어뜨림
        float opacityFade = distFade * distFade; 

        // 4. 그리드 그리기
        // fwidth가 너무 커지면(먼 거리) 자동으로 뭉개지도록 함
        float lineAlpha = getDynamicGrid(vBoxUv, currentWidth);
        
        if (lineAlpha > 0.01) {
            float gridStrength = lineAlpha * uBorderOpacity * opacityFade;

            if (gridStrength > 0.001) {
                float lum = getLuminance(finalColor);
                vec3 adaptiveColor = mix(vec3(1.0), vec3(0.0), smoothstep(0.3, 0.7, lum));
                finalColor = mix(finalColor, adaptiveColor, gridStrength);
            }
        }
    }

    outColor = vec4(finalColor, 1.0);

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

// ----------------------------------------------------------------------
// 4. Factory Function
// ----------------------------------------------------------------------

interface TileMaterialProps {
  channelTextures: Record<TileSurfaceKey, THREE.Texture>;
  elevationTexture: THREE.Texture;
  uResolution: THREE.Vector2;
  uVoxelSize: number;
  uLodScale: number;
  uEditMask?: THREE.Texture;
  gridConfig?: {
    uBorderColor: THREE.Color;
    uBorderOpacity: number;
  };
}

export const createTileMaterial = (props: TileMaterialProps) => {
  const { channelTextures, elevationTexture, uResolution, uVoxelSize, uLodScale, uEditMask, gridConfig } = props;

  const generatedColorUniforms: Record<string, any> = {};

  // 🛡️ 6자리 HEX(#RRGGBB)만 통과시키는 Regex 가드 유틸리티 사용
  Object.keys(BASE_TYPES).forEach((majorKey) => {
    const channelKey = majorKey.toLowerCase();
    const types = BASE_TYPES[majorKey];
    const colors = BASE_COLORS[majorKey];

    Object.keys(types).forEach((typeKey) => {
      const typeId = types?.[typeKey];
      const hexColor = colors?.[typeId];

      // 유효한 HEX 값일 때만 THREE.Color 인스턴스 생성 및 유니폼 등록
      if (ColorOps.isValidHex(hexColor)) {
        generatedColorUniforms[toColorUniformName(channelKey, typeKey)] = {
          value: new THREE.Color(hexColor),
        };
      }
    });
  });

  const defaultGrid = {
    uBorderColor: new THREE.Color('#000000'),
    uBorderOpacity: 0.04,
  };
  const activeGrid = gridConfig || defaultGrid;

  const uniforms: Record<string, any> = {
    ...THREE.UniformsLib.fog,
    uHeight: { value: elevationTexture },
    uVoxelZUnit: { value: CELL_SIZE },
    uResolution: { value: uResolution },
    uVoxelSize: { value: uVoxelSize },
    uLodScale: { value: uLodScale },
    uEditMask: {
      value: uEditMask || new THREE.DataTexture(new Uint8Array([0]), 1, 1),
    },
    uBorderColor: { value: activeGrid.uBorderColor },
    uBorderOpacity: { value: activeGrid.uBorderOpacity },
    ...generatedColorUniforms,
  };

  TILE_SURFACE_KEYS.forEach((key) => {
    if (channelTextures[key]) uniforms[toUniform(key)] = { value: channelTextures[key] };
  });

  return new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    vertexShader,
    fragmentShader: generateFragmentShader(),
    transparent: false,
    depthWrite: true,
    depthTest: true,
    side: THREE.DoubleSide,
    fog: true,
    extensions: {
      derivatives: true,
    } as any,
  });
};
