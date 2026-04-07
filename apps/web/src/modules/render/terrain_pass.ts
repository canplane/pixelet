// @pxl/web/modules/render/terrain_pass.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { F32Array } from "@xpute/spec/lib/kit/array.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L2_CENTER } from "@pxl/spec/space/indexer.ts";

import { GroundChannelKey, type Terrain } from "@pxl/bake/terrain/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import { page_to_chunks } from "@pxl/render/terrain/chunk.ts";

import { useTileStore } from "@/modules/world/terrain/tile-store.ts";

import type { RenderCameraState, RenderWorldState } from "@/modules/render/runtime.ts";

const ROW_STRIDE: u32 = CHUNK_CELL_SPAN;
const TOP_Z_BIAS: f64 = -0.005;
const TOP_THICKNESS_U: f64 = U_PER_CELL * 0.98;
const SKIRT_DROP_U: f64 = 200;

const SHADER_CODE = /* wgsl */ `
struct Uniforms {
  view_proj: mat4x4<f32>,
  world_info: vec4<f32>,
  local_world_offset: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VSIn {
  @location(0) position: vec3<f32>,
  @location(1) pos_kind: vec4<f32>,
  @location(2) scale_face: vec4<f32>,
};

struct VSOut {
  @builtin(position) position: vec4<f32>,
  @location(0) kind: f32,
  @location(1) face: f32,
};

fn rotate_z(v: vec3<f32>, azimuth: f32) -> vec3<f32> {
  let c = cos(azimuth);
  let s = sin(azimuth);
  return vec3<f32>(
    c * v.x - s * v.y,
    s * v.x + c * v.y,
    v.z,
  );
}

fn kind_color(kind: f32, face: f32) -> vec3<f32> {
  var color = vec3<f32>(0.48, 0.56, 0.33);

  if (kind > 2.5) {
    color = vec3<f32>(0.55, 0.55, 0.56);
  } else if (kind > 1.5) {
    color = vec3<f32>(0.62, 0.60, 0.56);
  } else if (kind > 0.5) {
    color = vec3<f32>(0.62, 0.82, 0.86);
  }

  if (face > 1.5) {
    return color * vec3<f32>(0.90, 0.87, 0.83);
  }
  if (face > 0.5) {
    return color * vec3<f32>(0.94, 0.91, 0.87);
  }
  return color * vec3<f32>(1.02, 1.01, 0.99);
}

@vertex
fn vs_main(in: VSIn) -> VSOut {
  let local = in.position * in.scale_face.xyz + in.pos_kind.xyz + u.local_world_offset.xyz;
  let world = rotate_z(local, u.world_info.x);

  var out: VSOut;
  out.position = u.view_proj * vec4<f32>(world, 1.0);
  out.kind = in.pos_kind.w;
  out.face = in.scale_face.w;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  return vec4<f32>(kind_color(in.kind, in.face), 1.0);
}
`;

const CUBE_VERTICES = new Float32Array([-0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5]);

const CUBE_INDICES = new Uint16Array([0, 1, 2, 2, 3, 0, 4, 6, 5, 6, 4, 7, 0, 4, 5, 5, 1, 0, 1, 5, 6, 6, 2, 1, 2, 6, 7, 7, 3, 2, 3, 7, 4, 4, 0, 3]);

const enum TerrainFaceType {
  TOP = 0,
  SIDE = 1,
  SKIRT = 2,
}

const enum TerrainCellType {
  LAND = 0,
  WATER = 1,
  STRUCTURE = 2,
  TRANSPORT = 3,
}

type ChunkBuffers = {
  pos_kind: GPUBuffer;
  scale_face: GPUBuffer;
  instance_cnt: u32;
};

type PageEntry = {
  terrain: Terrain;
  chunks: ChunkBuffers[];
};

export type TerrainPass = {
  device: GPUDevice;

  pipeline: GPURenderPipeline;
  bind_group: GPUBindGroup;
  uniform_buf: GPUBuffer;

  cube_vertex_buf: GPUBuffer;
  cube_idx_buf: GPUBuffer;
  cube_idx_cnt: u32;

  uniform_f32: F32Array;

  pages: Map<string, PageEntry>;
  prev_map_origin: IVec;
};

export function init_terrain_pass(device: GPUDevice, format: GPUTextureFormat): TerrainPass {
  const shader = device.createShaderModule({ code: SHADER_CODE });

  const uniform_f32 = new Float32Array(24);
  const uniform_buf = device.createBuffer({
    size: uniform_f32.byteLength,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });

  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: {
      module: shader,
      entryPoint: "vs_main",
      buffers: [
        {
          arrayStride: 12,
          stepMode: "vertex",
          attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }],
        },
        {
          arrayStride: 16,
          stepMode: "instance",
          attributes: [{ shaderLocation: 1, offset: 0, format: "float32x4" }],
        },
        {
          arrayStride: 16,
          stepMode: "instance",
          attributes: [{ shaderLocation: 2, offset: 0, format: "float32x4" }],
        },
      ],
    },
    fragment: {
      module: shader,
      entryPoint: "fs_main",
      targets: [{ format }],
    },
    primitive: {
      topology: "triangle-list",
      cullMode: "back",
    },
    depthStencil: {
      format: "depth24plus",
      depthWriteEnabled: true,
      depthCompare: "less",
    },
  });

  const bind_group = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniform_buf } }],
  });

  return {
    device,
    pipeline,
    bind_group,
    uniform_buf,
    cube_vertex_buf: create_static_buf(device, GPUBufferUsage.VERTEX, CUBE_VERTICES),
    cube_idx_buf: create_static_buf(device, GPUBufferUsage.INDEX, CUBE_INDICES),
    cube_idx_cnt: CUBE_INDICES.length,
    uniform_f32,
    pages: new Map<string, PageEntry>(),
    prev_map_origin: 0n,
  };
}

export function dispose_terrain_pass(pass_state: TerrainPass): void {
  for (const page of pass_state.pages.values()) {
    for (const chunk of page.chunks) {
      chunk.pos_kind.destroy();
      chunk.scale_face.destroy();
    }
  }
  pass_state.pages.clear();

  pass_state.uniform_buf.destroy();
  pass_state.cube_vertex_buf.destroy();
  pass_state.cube_idx_buf.destroy();
}

export function draw_terrain_pass(
  pass_state: TerrainPass,
  pass: GPURenderPassEncoder,
  camera: RenderCameraState,
  world: RenderWorldState,
): void {
  sync_pages(pass_state, world);
  write_uniforms(pass_state, camera, world);

  pass.setPipeline(pass_state.pipeline);
  pass.setBindGroup(0, pass_state.bind_group);
  pass.setVertexBuffer(0, pass_state.cube_vertex_buf);
  pass.setIndexBuffer(pass_state.cube_idx_buf, "uint16");

  for (const page of pass_state.pages.values()) {
    for (const chunk of page.chunks) {
      pass.setVertexBuffer(1, chunk.pos_kind);
      pass.setVertexBuffer(2, chunk.scale_face);
      pass.drawIndexed(pass_state.cube_idx_cnt, chunk.instance_cnt, 0, 0, 0);
    }
  }
}

function sync_pages(pass_state: TerrainPass, world: RenderWorldState): void {
  const tile_map = useTileStore.getState().tile_map;
  if (tile_map.size === 0) {
    clear_pages(pass_state);
    return;
  }

  const map_origin = world.map_origin;

  if (pass_state.prev_map_origin !== map_origin) {
    clear_pages(pass_state);
    pass_state.prev_map_origin = map_origin;
  }

  const next_keys = new Set<string>();

  for (const [page_idx, tile] of tile_map) {
    const key = page_idx.toString();
    next_keys.add(key);

    const prev = pass_state.pages.get(key);
    if (prev && prev.terrain === tile.terrain) {
      continue;
    }

    if (prev) {
      for (const chunk of prev.chunks) {
        chunk.pos_kind.destroy();
        chunk.scale_face.destroy();
      }
    }

    pass_state.pages.set(key, {
      terrain: tile.terrain,
      chunks: build_page_chunks(pass_state.device, page_idx, tile.terrain, map_origin),
    });
  }

  for (const [key, page] of pass_state.pages) {
    if (next_keys.has(key)) continue;
    for (const chunk of page.chunks) {
      chunk.pos_kind.destroy();
      chunk.scale_face.destroy();
    }
    pass_state.pages.delete(key);
  }
}

function clear_pages(pass_state: TerrainPass): void {
  for (const page of pass_state.pages.values()) {
    for (const chunk of page.chunks) {
      chunk.pos_kind.destroy();
      chunk.scale_face.destroy();
    }
  }
  pass_state.pages.clear();
}

function write_uniforms(
  pass_state: TerrainPass,
  camera: RenderCameraState,
  world: RenderWorldState,
): void {
  pass_state.uniform_f32.set(camera.view_proj, 0);
  pass_state.uniform_f32[16] = world.world_azimuth;
  pass_state.uniform_f32[17] = 0;
  pass_state.uniform_f32[18] = 0;
  pass_state.uniform_f32[19] = 0;
  pass_state.uniform_f32[20] = world.local_world_offset[0];
  pass_state.uniform_f32[21] = world.local_world_offset[1];
  pass_state.uniform_f32[22] = world.local_world_offset[2];
  pass_state.uniform_f32[23] = 0;

  pass_state.device.queue.writeBuffer(pass_state.uniform_buf, 0, pass_state.uniform_f32);
}

function create_static_buf(device: GPUDevice, usage: GPUBufferUsageFlags, data: Float32Array | Uint16Array): GPUBuffer {
  const buf = device.createBuffer({
    size: data.byteLength,
    usage,
    mappedAtCreation: true,
  });

  if (data instanceof Float32Array) {
    new Float32Array(buf.getMappedRange()).set(data);
  } else {
    new Uint16Array(buf.getMappedRange()).set(data);
  }

  buf.unmap();
  return buf;
}

function create_instance_buf(device: GPUDevice, data: Float32Array): GPUBuffer {
  const buf = device.createBuffer({
    size: data.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    mappedAtCreation: true,
  });

  new Float32Array(buf.getMappedRange()).set(data);
  buf.unmap();
  return buf;
}

function build_page_chunks(device: GPUDevice, page_idx: IVec, page: Terrain, map_origin: IVec): ChunkBuffers[] {
  const page_base = FROM_L3_IDX(page_idx);
  const local_page = ivec.sub(page_base, map_origin);

  const page_chunks = page_to_chunks(page.ground, page.ele);

  const out: ChunkBuffers[] = [];
  for (const [page_off, chunk] of page_chunks) {
    out.push(build_chunk(device, local_page, page_off, chunk, TerrainFaceType.TOP));
    out.push(build_chunk(device, local_page, page_off, chunk, TerrainFaceType.SIDE));
    out.push(build_chunk(device, local_page, page_off, chunk, TerrainFaceType.SKIRT));
  }
  return out;
}

function build_chunk(
  device: GPUDevice,
  local_page: IVec,
  page_off: IVec,
  chunk: Terrain,
  face: TerrainFaceType,
): ChunkBuffers {
  const cnt = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;
  const pos_kind = new Float32Array(cnt * 4);
  const scale_face = new Float32Array(cnt * 4);

  const chunk_center = L2_CENTER(page_off);
  const base_x = ivec.to_real(ivec.X(local_page)) + ivec.to_real(ivec.X(chunk_center));
  const base_y = ivec.to_real(ivec.Y(local_page)) + ivec.to_real(ivec.Y(chunk_center));
  const base_z = ivec.to_real(ivec.Z(local_page));

  const off_center = CHUNK_CELL_SPAN * 0.5;
  const { ground, ele } = chunk;

  for (let iy: u32 = 0; iy < CHUNK_CELL_SPAN; iy++) {
    for (let ix: u32 = 0; ix < CHUNK_CELL_SPAN; ix++) {
      const idx: u32 = iy * ROW_STRIDE + ix;
      const off: u32 = idx * 4;

      const surface_z: f64 = base_z + ele[idx] * U_PER_CELL;
      const kind = get_cell_kind(ground, idx);

      const ix_w = ix > 0 ? ix - 1 : ix;
      const ix_e = ix + 1 < CHUNK_CELL_SPAN ? ix + 1 : ix;
      const iy_s = iy > 0 ? iy - 1 : iy;
      const iy_n = iy + 1 < CHUNK_CELL_SPAN ? iy + 1 : iy;

      const h_w = base_z + ele[iy * ROW_STRIDE + ix_w] * U_PER_CELL;
      const h_e = base_z + ele[iy * ROW_STRIDE + ix_e] * U_PER_CELL;
      const h_s = base_z + ele[iy_s * ROW_STRIDE + ix] * U_PER_CELL;
      const h_n = base_z + ele[iy_n * ROW_STRIDE + ix] * U_PER_CELL;

      const cell_bottom_z = surface_z - U_PER_CELL;
      const side_bottom_z = Math.min(Math.min(h_w, h_e), Math.min(h_s, h_n)) - U_PER_CELL;

      const is_edge = ix === 0 || iy === 0 || ix === CHUNK_CELL_SPAN - 1 || iy === CHUNK_CELL_SPAN - 1;
      const skirt_bottom_z = is_edge ? surface_z - SKIRT_DROP_U : cell_bottom_z;

      const top_span_z = TOP_THICKNESS_U;
      const top_center_z = surface_z - top_span_z * 0.5 + TOP_Z_BIAS;

      const side_top_z = surface_z - top_span_z;
      const side_span_z = Math.max(0.01, side_top_z - side_bottom_z);
      const side_center_z = side_bottom_z + side_span_z * 0.5;

      const skirt_top_z = Math.min(side_bottom_z, side_top_z);
      const skirt_span_z = Math.max(0.01, skirt_top_z - skirt_bottom_z);
      const skirt_center_z = skirt_bottom_z + skirt_span_z * 0.5;

      const x = base_x + (ix - off_center + 0.5) * U_PER_CELL;
      const y = base_y + (iy - off_center + 0.5) * U_PER_CELL;

      let z = top_center_z;
      let span_z = top_span_z;

      if (face === TerrainFaceType.SIDE) {
        z = side_center_z;
        span_z = side_span_z;
      } else if (face === TerrainFaceType.SKIRT) {
        z = skirt_center_z;
        span_z = skirt_span_z;
      }

      pos_kind[off + 0] = x;
      pos_kind[off + 1] = y;
      pos_kind[off + 2] = z;
      pos_kind[off + 3] = kind;

      scale_face[off + 0] = U_PER_CELL;
      scale_face[off + 1] = U_PER_CELL;
      scale_face[off + 2] = span_z;
      scale_face[off + 3] = face;
    }
  }

  return {
    pos_kind: create_instance_buf(device, pos_kind),
    scale_face: create_instance_buf(device, scale_face),
    instance_cnt: cnt,
  };
}

function get_cell_kind(ground: Terrain["ground"], idx: u32): TerrainCellType {
  if (ground[GroundChannelKey.TRANSPORT][idx] > 0) return TerrainCellType.TRANSPORT;
  if (ground[GroundChannelKey.STRUCTURE][idx] > 0) return TerrainCellType.STRUCTURE;
  if (ground[GroundChannelKey.WATER][idx] > 0) return TerrainCellType.WATER;
  return TerrainCellType.LAND;
}
