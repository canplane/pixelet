// @pxl/web/render/gpu/model_asset.ts

/**
 * A loaded glTF handed to the module as a model (crates/pxl-core/src/model.rs):
 * the one place a parsed asset becomes the core's lanes, so a companion, a
 * decor kind or an asset a player brings all enter the world the same way.
 * The parse itself is still three-stdlib's GLTFLoader (gltf_cache.ts); only
 * this file reads its output, and it goes when the parser does.
 *
 * What is read: every skinned mesh of the scene, merged onto the first one's
 * skeleton, its vertices put through its bind matrix — the space the inverse
 * bind matrices expect — or, with no skin, every mesh baked through its node
 * transform onto one joint at rest. Each joint's parent, rest pose, the
 * transform above it when its parent is not a joint, and its inverse bind
 * matrix; each clip's channels on joints, by the node name its track binds.
 * The height the model is scaled to is measured as use_character_model.ts
 * measured it, the scene's box at rest.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

export type ModelHandle = {
  model: u32;
  /** Clip index by the name the glTF gives it. */
  clips: ReadonlyMap<string, u32>;
};

const PATHS: Record<string, u32> = { position: 0, quaternion: 1, scale: 2 };

const $m = new THREE.Matrix4();
const $n = new THREE.Matrix3();
const $v = new THREE.Vector3();

/** Registers `scene` and `animations` as a model standing `height_u` tall;
 * null when it has no mesh or the pool refused its block. */
export function register_model(pxl: PxlExports, scene: THREE.Object3D, animations: readonly THREE.AnimationClip[], height_u: f64): ModelHandle | null {
  scene.updateMatrixWorld(true);
  const skinned: THREE.SkinnedMesh[] = [];
  const plain: THREE.Mesh[] = [];
  scene.traverse((node) => {
    if ((node as THREE.SkinnedMesh).isSkinnedMesh) skinned.push(node as THREE.SkinnedMesh);
    else if ((node as THREE.Mesh).isMesh) plain.push(node as THREE.Mesh);
  });
  const meshes: THREE.Mesh[] = skinned.length > 0 ? skinned : plain;
  if (meshes.length === 0) return null;
  const bones: THREE.Bone[] = skinned.length > 0 ? skinned[0].skeleton.bones : [];
  const joint_cnt = Math.max(1, bones.length);

  let vertex_cnt = 0;
  let index_cnt = 0;
  for (const mesh of meshes) {
    const g = mesh.geometry;
    vertex_cnt += g.getAttribute("position").count;
    index_cnt += g.getIndex()?.count ?? g.getAttribute("position").count;
  }
  const id = pxl.pxl_model_new(vertex_cnt, index_cnt, joint_cnt);
  if (id < 0) return null;
  const model = id as u32;
  const block = pxl.pxl_model_block();
  const words = new Uint32Array(pxl.memory.buffer, block, vertex_cnt * 16 + index_cnt);
  const floats = new Float32Array(pxl.memory.buffer, block, vertex_cnt * 16 + index_cnt);
  const lane = (k: u32) => k * vertex_cnt;

  let v0 = 0;
  let i0 = 0;
  for (const mesh of meshes) {
    const g = mesh.geometry;
    const pos = g.getAttribute("position");
    const nrm = g.getAttribute("normal");
    const uv = g.getAttribute("uv");
    const sk_idx = g.getAttribute("skinIndex");
    const sk_w = g.getAttribute("skinWeight");
    const is_skinned = (mesh as THREE.SkinnedMesh).isSkinnedMesh;
    // Into the space the joints' inverse bind matrices expect: a skinned
    // mesh's bind matrix, a plain mesh's own node transform.
    $m.copy(is_skinned ? (mesh as THREE.SkinnedMesh).bindMatrix : mesh.matrixWorld);
    $n.getNormalMatrix($m);
    const remap = is_skinned ? (mesh as THREE.SkinnedMesh).skeleton.bones.map((b) => Math.max(0, bones.indexOf(b))) : null;
    for (let i = 0; i < pos.count; i++) {
      const v = v0 + i;
      $v.fromBufferAttribute(pos, i).applyMatrix4($m);
      floats[lane(0) + v * 3] = $v.x;
      floats[lane(0) + v * 3 + 1] = $v.y;
      floats[lane(0) + v * 3 + 2] = $v.z;
      if (nrm) $v.fromBufferAttribute(nrm, i).applyMatrix3($n).normalize();
      else $v.set(0, 1, 0);
      floats[lane(3) + v * 3] = $v.x;
      floats[lane(3) + v * 3 + 1] = $v.y;
      floats[lane(3) + v * 3 + 2] = $v.z;
      floats[lane(6) + v * 2] = uv ? uv.getX(i) : 0;
      floats[lane(6) + v * 2 + 1] = uv ? uv.getY(i) : 0;
      for (let c = 0; c < 4; c++) {
        words[lane(8) + v * 4 + c] = remap && sk_idx ? remap[sk_idx.getComponent(i, c)] ?? 0 : 0;
        floats[lane(12) + v * 4 + c] = sk_w ? sk_w.getComponent(i, c) : (c === 0 ? 1 : 0);
      }
    }
    const index = g.getIndex();
    const n = index ? index.count : pos.count;
    for (let i = 0; i < n; i++) words[lane(16) + i0 + i] = v0 + (index ? index.getX(i) : i);
    v0 += pos.count;
    i0 += n;
  }

  // Joints: parent, rest pose, what stands above a root, inverse bind.
  const row = FIXED_ALLOC.take(Float32Array, 42);
  const identity = new THREE.Matrix4();
  for (let j = 0; j < joint_cnt; j++) {
    const bone = bones[j];
    if (!bone) {
      row.set([0, 0, 0, 0, 0, 0, 1, 1, 1, 1], 0);
      row.set(identity.elements, 10);
      row.set(identity.elements, 26);
      pxl.pxl_model_joint(model, j, -1, row.byteOffset);
      continue;
    }
    const parent = bone.parent ? bones.indexOf(bone.parent as THREE.Bone) : -1;
    row.set([bone.position.x, bone.position.y, bone.position.z, bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w, bone.scale.x, bone.scale.y, bone.scale.z], 0);
    row.set(parent < 0 && bone.parent ? bone.parent.matrixWorld.elements : identity.elements, 10);
    row.set(skinned[0].skeleton.boneInverses[j].elements, 26);
    pxl.pxl_model_joint(model, j, parent, row.byteOffset);
  }
  FIXED_ALLOC.give(row);

  // Clips: the channels on joints.
  const clips = new Map<string, u32>();
  const by_name = new Map<string, u32>();
  bones.forEach((b, j) => {
    by_name.set(b.name, j);
    by_name.set(THREE.PropertyBinding.sanitizeNodeName(b.name), j);
  });
  for (const clip of animations) {
    const c = pxl.pxl_model_clip(model, clip.duration);
    clips.set(clip.name, c);
    for (const track of clip.tracks) {
      const parsed = THREE.PropertyBinding.parseTrackName(track.name);
      const joint = by_name.get(parsed.nodeName);
      const path = PATHS[parsed.propertyName];
      if (joint === undefined || path === undefined) continue;
      const width = path === 1 ? 4 : 3;
      const keys = track.times.length;
      let values = track.values as Float32Array;
      // Cubic spline keys carry in-tangent, value, out-tangent: the value.
      if (values.length === keys * width * 3) {
        const mid = new Float32Array(keys * width);
        for (let k = 0; k < keys; k++) mid.set(values.subarray(k * width * 3 + width, k * width * 3 + width * 2), k * width);
        values = mid;
      }
      const times_lane = FIXED_ALLOC.take(Float32Array, keys);
      const values_lane = FIXED_ALLOC.take(Float32Array, keys * width);
      times_lane.set(track.times);
      values_lane.set(values.subarray(0, keys * width));
      const step = track.getInterpolation() === THREE.InterpolateDiscrete ? 1 : 0;
      pxl.pxl_model_channel(model, c, joint, path, step, keys, times_lane.byteOffset, values_lane.byteOffset);
      FIXED_ALLOC.give(times_lane);
      FIXED_ALLOC.give(values_lane);
    }
  }

  const box = new THREE.Box3().setFromObject(scene);
  const raw = box.max.y - box.min.y;
  pxl.pxl_model_seal(model, raw > 0 ? height_u / raw : 1);
  return { model, clips };
}
