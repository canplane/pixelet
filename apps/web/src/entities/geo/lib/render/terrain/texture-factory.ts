/**
 * @file texture-factory.ts
 * 서버에서 전송된 바이너리 그리드(Uint8Array, Int16Array)를
 * GPU가 이해할 수 있는 Three.js DataTexture로 변환합니다.
 */

import * as THREE from 'three';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';

const { TILE_RES } = GEO_CONSTANTS;

/**
 * 🎯 Anisotropy 캐싱 변수
 * 렌더러 사양을 한 번 확인한 뒤에는 그 값을 계속 재사용합니다.
 */
let maxAnisotropy: number | null = null;

const getSafeAnisotropy = (renderer?: THREE.WebGLRenderer) => {
  if (maxAnisotropy !== null) return maxAnisotropy;

  if (renderer) {
    maxAnisotropy = Math.min(renderer.capabilities.getMaxAnisotropy(), 8);
    return maxAnisotropy;
  }

  return 1; // 렌더러가 아직 없을 때의 기본값
};

/**
 * [createChannelTexture]
 * ID 맵은 도트 질감 유지와 원거리 앨리어싱 방지를 위해 밉맵을 사용합니다.
 */
export const createChannelTexture = (data: Uint8Array, renderer?: THREE.WebGLRenderer): THREE.DataTexture => {
  const tex = new THREE.DataTexture(data, TILE_RES, TILE_RES, THREE.RedFormat, THREE.UnsignedByteType);
  tex.internalFormat = 'R8';

  // 🎯 [Optimization] 필터링과 래핑 설정을 간결하게 정리
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestMipmapLinearFilter;
  tex.generateMipmaps = true;

  // 🎯 렌더러 사양에 맞춘 비등방성 필터링 설정 (기울어진 면 품질 향상)
  tex.anisotropy = getSafeAnisotropy(renderer);

  tex.wrapS = tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
};

/**
 * [createElevationTexture]
 * 고도 데이터는 보간 없는 정밀 데이터를 위해 밉맵을 생성하지 않습니다.
 */
export const createElevationTexture = (data: Int16Array): THREE.DataTexture => {
  const tex = new THREE.DataTexture(data, TILE_RES, TILE_RES, THREE.RedIntegerFormat, THREE.ShortType);
  tex.internalFormat = 'R16I';

  // 🛡️ 고도 데이터는 무조건 인접 샘플링만 허용
  tex.magFilter = tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;

  tex.wrapS = tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
};
