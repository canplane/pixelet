// @pxl/web/modules/world/terrain/shader/tex.ts

import * as THREE from "three";

import type { u32 } from "@pixelet/shared/kit/word.ts";
import type { I16Array, U8Array } from "@pixelet/shared/kit/array.ts";

let max_anisotropy: number | null = null;

function get_safe_anisotropy(renderer?: THREE.WebGLRenderer): number {
  if (max_anisotropy !== null) return max_anisotropy;

  if (renderer) {
    max_anisotropy = Math.min(renderer.capabilities.getMaxAnisotropy(), 8);
    return max_anisotropy;
  }
  return 1;
}

export function create_splatmap(
  ch: U8Array,
  row_stride: u32,
  renderer?: THREE.WebGLRenderer,
): THREE.DataTexture {
  const tex = new THREE.DataTexture(ch, row_stride, row_stride, THREE.RedFormat, THREE.UnsignedByteType);
  tex.internalFormat = "R8";

  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.anisotropy = 1;

  // tex.magFilter = THREE.NearestFilter;
  // tex.minFilter = THREE.NearestMipmapLinearFilter;
  // tex.generateMipmaps = true;
  // tex.anisotropy = get_safe_anisotropy(renderer);

  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}

export function create_heightmap(ch: I16Array, row_stride: u32): THREE.DataTexture {
  const tex = new THREE.DataTexture(ch, row_stride, row_stride, THREE.RedIntegerFormat, THREE.ShortType);
  tex.internalFormat = "R16I";
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}
