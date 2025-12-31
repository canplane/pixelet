/**
 * @file texture-factory.ts
 * 서버에서 전송된 바이너리 그리드(Uint8Array, Uint16Array)를
 * GPU가 이해할 수 있는 Three.js DataTexture로 변환합니다.
 */

import * as THREE from 'three';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { TILE_RESOLUTION: resolution } = OSM_CONSTANTS;

/**
 * [createLayerTexture]
 * 8-bit 레이어 데이터(Sea, Base, Water...)를 텍스처로 변환합니다.
 */
export const createLayerTexture = (data: Uint8Array): THREE.DataTexture => {
  const tex = new THREE.DataTexture(
    data,
    resolution,
    resolution,
    THREE.RedFormat,
    THREE.UnsignedByteType,
  );
  // 🎯 WebGL 2.0 Red Channel 8-bit Integer 포맷 명시
  tex.internalFormat = 'R8';
  // 픽셀 아트 스타일(Voxel) 유지를 위해 보간(Linear) 대신 최근접(Nearest) 필터 사용
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;

  // 텍스처 데이터 업로드 플래그
  tex.needsUpdate = true;
  return tex;
};

/**
 * [createTerrainTexture]
 * 16-bit 고도 데이터를 텍스처로 변환합니다.
 */
export const createTerrainTexture = (data: Uint16Array): THREE.DataTexture => {
  const tex = new THREE.DataTexture(
    data,
    resolution,
    resolution,
    THREE.RedIntegerFormat,
    THREE.UnsignedShortType,
  );
  // 🎯 WebGL 2.0 16-bit Unsigned Integer 포맷 (0~65535 정밀도)
  tex.internalFormat = 'R16UI';
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;

  tex.needsUpdate = true;
  return tex;
};
