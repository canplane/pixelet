// @pxl/web/render/gltf/gltf.ts

/**
 * A glTF 2.0 parser for what the shipped models use: a GLB container or a
 * JSON document with its buffers already in hand; nodes with TRS or a
 * matrix; meshes of triangle primitives with positions, normals, texture
 * coordinates, joints and weights; skins with inverse bind matrices; and
 * animations of translation, rotation and scale with linear, step or
 * cubic spline keys. Images and materials are read only far enough to say
 * which texture a primitive paints with, since every model here is skinned
 * from a swatch the palette names rather than the file's own image.
 *
 * Nothing here is Y-up or Z-up: the vertices, poses and keys come out as
 * the file holds them, in the file's own frame, and whoever registers the
 * model (render/gpu/model_asset.ts) decides how it stands.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { type Mat4, mat4, mat4_mul } from "@xpute/core/kit/mat4.ts";

export type GltfPrimitive = {
  position: Float32Array;
  normal: Float32Array | null;
  uv: Float32Array | null;
  /** Four joint indices a vertex, into the mesh's skin; null when unskinned. */
  joints: Uint16Array | null;
  weights: Float32Array | null;
  /** Triangle list; generated as 0..n when the primitive has none. */
  index: Uint32Array;
  material: number;
};

export type GltfNode = {
  name: string;
  parent: number;
  children: readonly number[];
  mesh: number;
  skin: number;
  /** The node's own transform, column-major. */
  local: Mat4;
  translation: [f64, f64, f64];
  rotation: [f64, f64, f64, f64];
  scale: [f64, f64, f64];
};

export type GltfSkin = {
  joints: readonly number[];
  /** Sixteen floats a joint, column-major; identity when the file gives none. */
  inverse_bind: Float32Array;
};

export const enum GltfPath {
  TRANSLATION = 0,
  ROTATION = 1,
  SCALE = 2,
}

export type GltfChannel = {
  node: number;
  path: GltfPath;
  /** LINEAR or CUBICSPLINE keys interpolate; STEP holds. */
  step: boolean;
  times: Float32Array;
  /** `times.length` keys of 3 or 4 floats; a cubic spline's tangents are
   * already dropped, leaving the values. */
  values: Float32Array;
};

export type GltfAnimation = {
  name: string;
  duration: f64;
  channels: GltfChannel[];
};

export type Gltf = {
  nodes: GltfNode[];
  meshes: GltfPrimitive[][];
  skins: GltfSkin[];
  animations: GltfAnimation[];
  /** The default scene's root nodes. */
  roots: readonly number[];
};

const GLB_MAGIC = 0x46546c67;
const CHUNK_JSON = 0x4e4f534a;
const CHUNK_BIN = 0x004e4942;

const COMPONENT_SIZE: Record<number, number> = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 };
const TYPE_SIZE: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT2: 4, MAT3: 9, MAT4: 16 };

// deno-lint-ignore no-explicit-any
type Json = any;

/** Parses a `.glb`, or a `.gltf` document whose buffers are `buffers`. */
export function parse_gltf(bytes: ArrayBuffer, external_buffers: readonly ArrayBuffer[] = []): Gltf {
  const view = new DataView(bytes);
  let json: Json;
  let buffers: ArrayBuffer[];
  if (bytes.byteLength >= 12 && view.getUint32(0, true) === GLB_MAGIC) {
    const length = Math.min(view.getUint32(8, true), bytes.byteLength);
    let at = 12;
    let text: string | null = null;
    let bin: ArrayBuffer | null = null;
    while (at + 8 <= length) {
      const chunk_length = view.getUint32(at, true);
      const chunk_type = view.getUint32(at + 4, true);
      const data = bytes.slice(at + 8, at + 8 + chunk_length);
      if (chunk_type === CHUNK_JSON) text = new TextDecoder().decode(data);
      else if (chunk_type === CHUNK_BIN && bin === null) bin = data;
      at += 8 + chunk_length;
    }
    if (text === null) throw new Error("gltf: a GLB without a JSON chunk");
    json = JSON.parse(text);
    buffers = bin ? [bin, ...external_buffers] : [...external_buffers];
  } else {
    json = JSON.parse(new TextDecoder().decode(bytes));
    buffers = [...external_buffers];
  }
  return read_document(json, buffers);
}

function read_document(json: Json, buffers: ArrayBuffer[]): Gltf {
  const accessor_cache = new Map<number, Float32Array | Uint32Array | Uint16Array>();

  /** An accessor's elements, tightly packed, as floats or as integers. */
  function read_accessor(index: number, as: "f32" | "u32" | "u16"): Float32Array | Uint32Array | Uint16Array {
    const key = index * 4 + (as === "f32" ? 0 : as === "u32" ? 1 : 2);
    const cached = accessor_cache.get(key);
    if (cached) return cached;
    const a = json.accessors[index];
    const n = a.count * TYPE_SIZE[a.type];
    const out = as === "f32" ? new Float32Array(n) : as === "u32" ? new Uint32Array(n) : new Uint16Array(n);
    if (a.bufferView !== undefined) {
      const bv = json.bufferViews[a.bufferView];
      const buffer = buffers[bv.buffer];
      if (!buffer) throw new Error(`gltf: buffer ${bv.buffer} not provided`);
      const comp = COMPONENT_SIZE[a.componentType];
      const width = TYPE_SIZE[a.type];
      const stride = bv.byteStride ?? comp * width;
      const base = (bv.byteOffset ?? 0) + (a.byteOffset ?? 0);
      const dv = new DataView(buffer);
      const norm = a.normalized === true;
      for (let e = 0; e < a.count; e++) {
        for (let c = 0; c < width; c++) {
          const at = base + e * stride + c * comp;
          let v: number;
          switch (a.componentType) {
            case 5120:
              v = dv.getInt8(at);
              if (norm) v = Math.max(v / 127, -1);
              break;
            case 5121:
              v = dv.getUint8(at);
              if (norm) v /= 255;
              break;
            case 5122:
              v = dv.getInt16(at, true);
              if (norm) v = Math.max(v / 32767, -1);
              break;
            case 5123:
              v = dv.getUint16(at, true);
              if (norm) v /= 65535;
              break;
            case 5125:
              v = dv.getUint32(at, true);
              break;
            default:
              v = dv.getFloat32(at, true);
          }
          out[e * width + c] = v;
        }
      }
    }
    // Sparse accessors are not in the shipped models; a sparse one reads
    // as its dense base, which the spec allows to be all zero.
    accessor_cache.set(key, out);
    return out;
  }

  const meshes: GltfPrimitive[][] = (json.meshes ?? []).map((m: Json) =>
    (m.primitives as Json[])
      .filter((p) => (p.mode ?? 4) === 4 && p.attributes.POSITION !== undefined)
      .map((p): GltfPrimitive => {
        const position = read_accessor(p.attributes.POSITION, "f32") as Float32Array;
        const count = position.length / 3;
        const index = p.indices !== undefined ? read_accessor(p.indices, "u32") as Uint32Array : Uint32Array.from({ length: count }, (_, i) => i);
        return {
          position,
          normal: p.attributes.NORMAL !== undefined ? read_accessor(p.attributes.NORMAL, "f32") as Float32Array : null,
          uv: p.attributes.TEXCOORD_0 !== undefined ? read_accessor(p.attributes.TEXCOORD_0, "f32") as Float32Array : null,
          joints: p.attributes.JOINTS_0 !== undefined ? read_accessor(p.attributes.JOINTS_0, "u16") as Uint16Array : null,
          weights: p.attributes.WEIGHTS_0 !== undefined ? read_accessor(p.attributes.WEIGHTS_0, "f32") as Float32Array : null,
          index,
          material: p.material ?? -1,
        };
      })
  );

  const raw_nodes: Json[] = json.nodes ?? [];
  const parents = new Int32Array(raw_nodes.length).fill(-1);
  raw_nodes.forEach((n, i) => {
    for (const c of n.children ?? []) parents[c] = i;
  });
  const nodes: GltfNode[] = raw_nodes.map((n, i): GltfNode => {
    const translation: [f64, f64, f64] = n.translation ?? [0, 0, 0];
    const rotation: [f64, f64, f64, f64] = n.rotation ?? [0, 0, 0, 1];
    const scale: [f64, f64, f64] = n.scale ?? [1, 1, 1];
    const local = mat4();
    if (n.matrix) local.set(n.matrix);
    else compose_trs(local, translation, rotation, scale);
    return { name: n.name ?? "", parent: parents[i], children: n.children ?? [], mesh: n.mesh ?? -1, skin: n.skin ?? -1, local, translation, rotation, scale };
  });

  const skins: GltfSkin[] = (json.skins ?? []).map((s: Json): GltfSkin => {
    const joints: number[] = s.joints;
    let inverse_bind: Float32Array;
    if (s.inverseBindMatrices !== undefined) inverse_bind = read_accessor(s.inverseBindMatrices, "f32") as Float32Array;
    else {
      inverse_bind = new Float32Array(joints.length * 16);
      for (let j = 0; j < joints.length; j++) inverse_bind.set([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], j * 16);
    }
    return { joints, inverse_bind };
  });

  const animations: GltfAnimation[] = (json.animations ?? []).map((a: Json, k: number): GltfAnimation => {
    let duration = 0;
    const channels: GltfChannel[] = [];
    for (const c of a.channels as Json[]) {
      const target = c.target;
      if (target.node === undefined) continue;
      const path = target.path === "translation" ? GltfPath.TRANSLATION : target.path === "rotation" ? GltfPath.ROTATION : target.path === "scale" ? GltfPath.SCALE : null;
      if (path === null) continue;
      const sampler = a.samplers[c.sampler];
      const times = read_accessor(sampler.input, "f32") as Float32Array;
      let values = read_accessor(sampler.output, "f32") as Float32Array;
      const width = path === GltfPath.ROTATION ? 4 : 3;
      const interpolation: string = sampler.interpolation ?? "LINEAR";
      if (interpolation === "CUBICSPLINE") {
        // In-tangent, value, out-tangent per key: the value.
        const mid = new Float32Array(times.length * width);
        for (let t = 0; t < times.length; t++) mid.set(values.subarray(t * width * 3 + width, t * width * 3 + width * 2), t * width);
        values = mid;
      }
      if (times.length > 0) duration = Math.max(duration, times[times.length - 1]);
      channels.push({ node: target.node, path, step: interpolation === "STEP", times, values: values.subarray(0, times.length * width) });
    }
    return { name: a.name ?? `animation_${k}`, duration, channels };
  });

  const scene = json.scenes?.[json.scene ?? 0];
  const roots: number[] = scene?.nodes ?? raw_nodes.map((_, i) => i).filter((i) => parents[i] < 0);
  return { nodes, meshes, skins, animations, roots };
}

/** T × R × S, column-major, from a translation, a unit quaternion and a scale. */
export function compose_trs(out: Mat4, t: readonly number[], q: readonly number[], s: readonly number[]): Mat4 {
  const [x, y, z, w] = q;
  const xx = x * x, yy = y * y, zz = z * z, xy = x * y, xz = x * z, yz = y * z, wx = w * x, wy = w * y, wz = w * z;
  out[0] = (1 - 2 * (yy + zz)) * s[0];
  out[1] = 2 * (xy + wz) * s[0];
  out[2] = 2 * (xz - wy) * s[0];
  out[3] = 0;
  out[4] = 2 * (xy - wz) * s[1];
  out[5] = (1 - 2 * (xx + zz)) * s[1];
  out[6] = 2 * (yz + wx) * s[1];
  out[7] = 0;
  out[8] = 2 * (xz + wy) * s[2];
  out[9] = 2 * (yz - wx) * s[2];
  out[10] = (1 - 2 * (xx + yy)) * s[2];
  out[11] = 0;
  out[12] = t[0];
  out[13] = t[1];
  out[14] = t[2];
  out[15] = 1;
  return out;
}

/** Every node's world matrix at rest, parents before children. */
export function world_matrices(gltf: Gltf): Mat4[] {
  const out: Mat4[] = gltf.nodes.map(() => mat4());
  const visit = (i: number, parent: Mat4 | null) => {
    if (parent) mat4_mul(out[i], parent, gltf.nodes[i].local);
    else out[i].set(gltf.nodes[i].local);
    for (const c of gltf.nodes[i].children) visit(c, out[i]);
  };
  for (const r of gltf.roots) visit(r, null);
  return out;
}

/** The nodes reachable from the scene's roots, parents before children. */
export function scene_nodes(gltf: Gltf): u32[] {
  const out: u32[] = [];
  const visit = (i: number) => {
    out.push(i);
    for (const c of gltf.nodes[i].children) visit(c);
  };
  for (const r of gltf.roots) visit(r);
  return out;
}
