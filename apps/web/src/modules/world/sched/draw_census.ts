// @pxl/web/modules/world/sched/draw_census.ts

/**
 * Draws and triangles per kind of thing drawn, for the ledger.
 *
 * `gl.info` gives one total, which says the GPU is busy but not with what:
 * 2.7M triangles at 13.8 ms could be the finest terrain tier, a blend
 * partner drawn twice, or decor instances, and each wants a different fix.
 * This wraps `renderBufferDirect` — the one call every draw goes through,
 * the shadow map's included — and files each draw under a tag read off the
 * object's name, which chunk_node.ts (`primary/lod/4`), decor/field.ts
 * (`decor/…`) and the sky already set.
 *
 * Triangles are counted the way three counts them for `gl.info`: the index
 * (or position) count clipped to the draw range and the material group,
 * times the instance count. Dev only; the wrapper is a map lookup and a few
 * adds per draw.
 */

import type * as THREE from "three";

/** tag → [draws, triangles], for one frame's render. */
export type DrawCensus = Record<string, [number, number]>;

const LOD_NAME = /^(?:primary|secondary)\/lod\/(\d+)$/;
/** decor/field.ts names a bank `decor/<kind>:<variant>`, `#hidden` for the
 * hit-only twin that is never drawn. */
const DECOR_NAME = /^decor\/([^:]+):/;

const tag_cache = new WeakMap<THREE.Object3D, string>();

function tag_of(object: THREE.Object3D): string {
  let tag = tag_cache.get(object);
  if (tag !== undefined) return tag;
  const lod = LOD_NAME.exec(object.name);
  if (lod) tag = `lod${lod[1]}`;
  else if (DECOR_NAME.test(object.name)) tag = `decor:${DECOR_NAME.exec(object.name)![1]}`;
  else if ((object as THREE.Sprite).isSprite) tag = "badge";
  else if (object.name === "sky") tag = "sky";
  else tag = "other";
  tag_cache.set(object, tag);
  return tag;
}

function triangles_of(geometry: THREE.BufferGeometry, object: THREE.Object3D, group: { start: number; count: number } | null): number {
  if ((object as THREE.Sprite).isSprite) return 2;
  if (!(object as THREE.Mesh).isMesh) return 0;
  const index = geometry.index;
  const total = index !== null ? index.count : geometry.attributes.position.count;
  let start = geometry.drawRange.start;
  let end = geometry.drawRange.start + geometry.drawRange.count;
  if (group !== null) {
    start = Math.max(start, group.start);
    end = Math.min(end, group.start + group.count);
  }
  const count = Math.max(0, Math.min(end, total) - Math.max(start, 0));
  const instances = (object as THREE.InstancedMesh).isInstancedMesh
    ? (object as THREE.InstancedMesh).count
    : (geometry as THREE.InstancedBufferGeometry).isInstancedBufferGeometry
    ? (geometry as THREE.InstancedBufferGeometry).instanceCount
    : 1;
  return Math.floor(count / 3) * instances;
}

let current = new Map<string, [number, number]>();

function tally(tag: string, triangles: number): void {
  const entry = current.get(tag);
  if (entry) {
    entry[0]++;
    entry[1] += triangles;
  } else current.set(tag, [1, triangles]);
}

/** Wraps the renderer; returns the unwrap. */
export function draw_census_install(gl: THREE.WebGLRenderer): () => void {
  const original = gl.renderBufferDirect;
  gl.renderBufferDirect = function (camera, scene, geometry, material, object, group) {
    // The shadow map passes a null scene — the same geometry drawn again
    // from the light, which is exactly the cost worth seeing separately.
    tally(scene === null ? "shadow" : tag_of(object), triangles_of(geometry, object, group));
    original.call(this, camera, scene, geometry, material, object, group);
  };
  return () => {
    gl.renderBufferDirect = original;
  };
}

/** Everything drawn since the last take, and a fresh count from here. */
export function draw_census_take(): DrawCensus {
  const out = Object.fromEntries(current);
  current = new Map();
  return out;
}
