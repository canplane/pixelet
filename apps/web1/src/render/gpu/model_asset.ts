// @pxl/web/render/gpu/model_asset.ts

/**
 * A parsed glTF (render/gltf/gltf.ts) handed to the module as a model
 * (crates/pxl/core/src/world/model.rs): the one place a parsed asset becomes the
 * core's lanes, so a companion, a decor kind or an asset a player brings
 * all enter the world the same way.
 *
 * A skinned model's joints are its skins' joint nodes, every primitive
 * merged onto that one list: its vertices are in the skin's space, as the
 * spec has them, and a joint's palette entry is its global transform times
 * its inverse bind matrix. A model with no skin is animated by its nodes
 * instead, so every node in the scene is a joint with an identity inverse
 * bind, and a mesh's vertices ride the joint of the node that holds it —
 * which is what plays a pet's walk, where each part is a node of its own.
 * A root joint whose parent is not a joint carries that parent's transform
 * above it. Each clip's channels on joints go over by node.
 *
 * The height the model is scaled to is measured over its vertices at
 * rest, through their nodes, as the box three drew around the scene.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { type Mat4, mat4_transform_point } from "@xpute/core/kit/mat4.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { FIXED_ALLOC } from "@/wasm/core_pool.ts";
import { type Gltf, scene_nodes, world_matrices } from "@/render/gltf/gltf.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

export type ModelHandle = {
  model: u32;
  /** Clip index by the name the glTF gives it. */
  clips: ReadonlyMap<string, u32>;
  /** Triangles, for the ledger. */
  triangles: u32;
};

const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
const $p = { x: 0, y: 0, z: 0 };

/** Registers `gltf` as a model standing `height_u` tall, or at its own
 * size when null; null when it has no mesh or the pool refused its block. */
export function register_model(pxl: PxlExports, gltf: Gltf, height_u: f64 | null): ModelHandle | null {
  const order = scene_nodes(gltf);
  const worlds = world_matrices(gltf);
  const skinned = gltf.skins.length > 0 && order.some((n) => gltf.nodes[n].skin >= 0 && gltf.nodes[n].mesh >= 0);

  // The joints: the skins' joint nodes in first-seen order, or every node.
  const joint_nodes: u32[] = [];
  const joint_of = new Map<u32, u32>();
  const add_joint = (node: u32) => {
    if (joint_of.has(node)) return;
    joint_of.set(node, joint_nodes.length);
    joint_nodes.push(node);
  };
  if (skinned) {
    for (const n of order) {
      const node = gltf.nodes[n];
      if (node.skin >= 0 && node.mesh >= 0) { for (const j of gltf.skins[node.skin].joints) add_joint(j); }
    }
  } else {
    for (const n of order) add_joint(n);
  }
  const joint_cnt = Math.max(1, joint_nodes.length);

  // The meshes: every primitive of every node that holds one, skinned or
  // not as the model is.
  const parts: { node: u32; prim: number; mesh: number }[] = [];
  let vertex_cnt = 0;
  let index_cnt = 0;
  for (const n of order) {
    const node = gltf.nodes[n];
    if (node.mesh < 0) continue;
    if (skinned && node.skin < 0) continue;
    gltf.meshes[node.mesh].forEach((prim, k) => {
      parts.push({ node: n, prim: k, mesh: node.mesh });
      vertex_cnt += prim.position.length / 3;
      index_cnt += prim.index.length;
    });
  }
  if (vertex_cnt === 0) return null;

  let block = 0;
  const id = CORE_BUILD.call(Cmd.MODEL_CREATE, packet((b) => b.u32(vertex_cnt).u32(index_cnt).u32(joint_cnt)), (made, reply) => {
    if (made >= 0 && reply !== null) block = reply.at(0).get<number>();
    return made;
  });
  if (id < 0) return null;
  const model = id as u32;
  const words = new Uint32Array(pxl.memory.buffer, block, vertex_cnt * 16 + index_cnt);
  const floats = new Float32Array(pxl.memory.buffer, block, vertex_cnt * 16 + index_cnt);
  const lane = (k: u32) => k * vertex_cnt;

  let lo = Infinity, hi = -Infinity;
  let v0 = 0;
  let i0 = 0;
  for (const part of parts) {
    const node = gltf.nodes[part.node];
    const prim = gltf.meshes[part.mesh][part.prim];
    const count = prim.position.length / 3;
    // A skinned vertex is in the skin's space already, and its joints are
    // the skin's; an unskinned one is the node's, whose joint places it.
    const remap = skinned && node.skin >= 0 ? gltf.skins[node.skin].joints.map((j) => joint_of.get(j) ?? 0) : null;
    const own_joint = joint_of.get(part.node) ?? 0;
    const world: Mat4 = worlds[part.node];
    for (let i = 0; i < count; i++) {
      const v = v0 + i;
      const x = prim.position[i * 3], y = prim.position[i * 3 + 1], z = prim.position[i * 3 + 2];
      floats[lane(0) + v * 3] = x;
      floats[lane(0) + v * 3 + 1] = y;
      floats[lane(0) + v * 3 + 2] = z;
      if (prim.normal) {
        floats[lane(3) + v * 3] = prim.normal[i * 3];
        floats[lane(3) + v * 3 + 1] = prim.normal[i * 3 + 1];
        floats[lane(3) + v * 3 + 2] = prim.normal[i * 3 + 2];
      } else {
        floats[lane(3) + v * 3] = 0;
        floats[lane(3) + v * 3 + 1] = 1;
        floats[lane(3) + v * 3 + 2] = 0;
      }
      floats[lane(6) + v * 2] = prim.uv ? prim.uv[i * 2] : 0;
      floats[lane(6) + v * 2 + 1] = prim.uv ? prim.uv[i * 2 + 1] : 0;
      for (let c = 0; c < 4; c++) {
        if (remap && prim.joints) {
          words[lane(8) + v * 4 + c] = remap[prim.joints[i * 4 + c]] ?? 0;
          floats[lane(12) + v * 4 + c] = prim.weights ? prim.weights[i * 4 + c] : (c === 0 ? 1 : 0);
        } else {
          words[lane(8) + v * 4 + c] = c === 0 ? own_joint : 0;
          floats[lane(12) + v * 4 + c] = c === 0 ? 1 : 0;
        }
      }
      // The height, measured where the vertex stands at rest.
      const at = remap ? $p : mat4_transform_point($p, world, x, y, z);
      if (remap) {
        at.x = x;
        at.y = y;
        at.z = z;
      }
      if (at.y < lo) lo = at.y;
      if (at.y > hi) hi = at.y;
    }
    for (let i = 0; i < prim.index.length; i++) words[lane(16) + i0 + i] = v0 + prim.index[i];
    v0 += count;
    i0 += prim.index.length;
  }

  // Joints: parent, rest pose, what stands above a root, inverse bind.
  const row = FIXED_ALLOC.take(Float32Array, 42);
  const inverse_bind_of = new Map<u32, Float32Array>();
  if (skinned) {
    for (const skin of gltf.skins) skin.joints.forEach((j, k) => inverse_bind_of.set(j, skin.inverse_bind.subarray(k * 16, k * 16 + 16)));
  }
  for (let j = 0; j < joint_cnt; j++) {
    const n = joint_nodes[j];
    if (n === undefined) {
      row.set([0, 0, 0, 0, 0, 0, 1, 1, 1, 1], 0);
      row.set(IDENTITY, 10);
      row.set(IDENTITY, 26);
      CORE_BUILD.send(model, Cmd.MODEL_JOINT, packet((b) => b.u32(j).i32(-1).f32_array(row)));
      continue;
    }
    const node = gltf.nodes[n];
    const parent_node = node.parent;
    const parent = parent_node >= 0 ? joint_of.get(parent_node) ?? -1 : -1;
    row.set([...node.translation, ...node.rotation, ...node.scale], 0);
    // A root joint under a node that is not one carries that node's world
    // transform; a scene root carries nothing.
    if (parent < 0 && parent_node >= 0) row.set(worlds[parent_node], 10);
    else row.set(IDENTITY, 10);
    row.set(inverse_bind_of.get(n) ?? IDENTITY, 26);
    CORE_BUILD.send(model, Cmd.MODEL_JOINT, packet((b) => b.u32(j).i32(parent).f32_array(row)));
  }
  FIXED_ALLOC.give(row);

  // Clips: the channels on joints.
  const clips = new Map<string, u32>();
  for (const anim of gltf.animations) {
    const c = CORE_BUILD.call(Cmd.MODEL_CLIP, packet((b) => b.u32(model).f32(anim.duration)), (index) => index >>> 0);
    clips.set(anim.name, c);
    for (const channel of anim.channels) {
      const joint = joint_of.get(channel.node);
      if (joint === undefined) continue;
      const width = channel.path === 1 ? 4 : 3;
      const keys = channel.times.length;
      const times = Float32Array.from(channel.times);
      const values = Float32Array.from(channel.values.subarray(0, keys * width));
      CORE_BUILD.send(model, Cmd.MODEL_CHANNEL, packet((b) => b.u32(c).u32(joint).u32(channel.path).bool(channel.step).f32_array(times).f32_array(values)));
    }
  }

  const raw = hi - lo;
  CORE_BUILD.send(model, Cmd.MODEL_SEAL, packet((b) => b.f32(height_u !== null && raw > 0 ? height_u / raw : 1)));
  return { model, clips, triangles: index_cnt / 3 };
}
