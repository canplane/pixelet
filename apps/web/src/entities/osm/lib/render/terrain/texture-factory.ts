/**
 * @file texture-factory.ts
 * 서버에서 전송된 바이너리 그리드(Uint8Array, Uint16Array)를
 * GPU가 이해할 수 있는 Three.js DataTexture로 변환합니다.
 */

import * as THREE from 'three';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { TILE_RES } = OSM_CONSTANTS;

// 🎯 성능과 품질 타협을 위한 비등방성 필터링 레벨 (보통 4~16 사이 값 사용)
// renderer.capabilities.getMaxAnisotropy() 값을 받아와서 쓰는 것이 가장 좋습니다.
const ANISOTROPY_LEVEL = 8; // 비등방성 필터링 (기울어진 면 품질 향상)

/**
 * [createChannelTexture]
 * ID 맵(정수형 데이터)은 보간되면 안 되므로 Nearest 필터를 사용하고,
 * 값이 섞이는 것을 막기 위해 Mipmap 생성을 비활성화합니다.
 */
export const createChannelTexture = (data: Uint8Array): THREE.DataTexture => {
  const tex = new THREE.DataTexture(
    data,
    TILE_RES,
    TILE_RES,
    THREE.RedFormat,
    THREE.UnsignedByteType,
  );
  tex.internalFormat = 'R8';

  // 🎯 [Fix] 자글거림 해결의 핵심
  // NearestFilter (X) -> 멀리서 픽셀 튐
  // NearestMipmapLinearFilter (O) -> 도트 느낌 유지 + 원거리 부드러움
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestMipmapLinearFilter;

  tex.generateMipmaps = true;
  tex.anisotropy = ANISOTROPY_LEVEL;

  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
};

/**
 * [createElevationTexture]
 * 고도 데이터 역시 정확한 값을 위해 Nearest를 유지합니다.
 */
export const createElevationTexture = (
  data: Uint16Array,
): THREE.DataTexture => {
  const tex = new THREE.DataTexture(
    data,
    TILE_RES,
    TILE_RES,
    THREE.RedIntegerFormat,
    THREE.UnsignedShortType,
  );
  tex.internalFormat = 'R16UI';

  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;

  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;

  tex.needsUpdate = true;
  return tex;
};
