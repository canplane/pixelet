// @pxl/web/render/overlay/user_marker/pass.ts

import type { f32, f64, i32, u32 } from "@xpute/spec/lib/kit/word.ts";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { ivec, projection } from "@pxl/spec/space/pvf3/mod.ts";
import { add3, center3n, copy3, floor3n, set3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import * as epsg4326 from "@pxl/spec/space/adapter/4326.ts";

import { useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { get_ele } from "@/modules/world/terrain/use-elevation.ts";
import { get_theme_key, WorldThemeKey } from "@/modules/world/render/themes.ts";
import type { AnchorRegistry } from "@/render/anchor/registry.ts";
import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";
import { init_overlay_uniform_state, type OverlayUniformState, write_overlay_uniform_state } from "@/render/overlay/types.ts";

import type { UserMarkerBeaconInstance, UserMarkerZoneInstance, ZoneSample } from "./types.ts";
import user_marker_wgsl from "./user_marker.wgsl?raw";

const ZONE_RADIUS_CELL: f64 = 16 / U_PER_CELL - .707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;

const ZONE_SAMPLES: ZoneSample[] = (() => {
  const range: i32 = Math.ceil(ZONE_RADIUS_CELL);
  const out: ZoneSample[] = [];

  for (let x: i32 = -range; x <= range; x++) {
    for (let y: i32 = -range; y <= range; y++) {
      const dx: f64 = x;
      const dy: f64 = y;
      const dx2: f64 = dx * dx;
      const dy2: f64 = dy * dy;

      if (dx2 + dy2 > ZONE_RADIUS2_CELL) continue;

      const d_top: f64 = dx2 + (dy + 1) * (dy + 1);
      const d_bottom: f64 = dx2 + (dy - 1) * (dy - 1);
      const d_right: f64 = (dx + 1) * (dx + 1) + dy2;
      const d_left: f64 = (dx - 1) * (dx - 1) + dy2;

      out.push({
        off: [dx * U_PER_CELL, dy * U_PER_CELL, -U_PER_CELL],
        borders: [
          d_top > ZONE_RADIUS2_CELL ? 1 : 0,
          d_bottom > ZONE_RADIUS2_CELL ? 1 : 0,
          d_right > ZONE_RADIUS2_CELL ? 1 : 0,
          d_left > ZONE_RADIUS2_CELL ? 1 : 0,
        ],
      });
    }
  }

  return out;
})();

const BEACON_Z_BIAS: f64 = 1;

const $0: V3 = [0, 0, 0];
const $1: V3 = [0, 0, 0];
const $2: V3 = [0, 0, 0];

type BeaconDrawState = {
  pipeline: GPURenderPipeline;
  uniform: OverlayUniformState;

  pos_scale_buf: GPUBuffer | null;
  color_buf: GPUBuffer | null;

  instance_cnt: u32;
  instance_cap: u32;
};

type ZoneDrawState = {
  pipeline: GPURenderPipeline;
  uniform: OverlayUniformState;

  pos_scale_buf: GPUBuffer | null;
  color_buf: GPUBuffer | null;
  borders_buf: GPUBuffer | null;

  instance_cnt: u32;
  instance_cap: u32;
};

export type UserMarkerPass = {
  device: GPUDevice;

  beacon: BeaconDrawState;
  zone: ZoneDrawState;
};

export function init_user_marker_pass(device: GPUDevice, format: GPUTextureFormat): UserMarkerPass {
  const shader = device.createShaderModule({ code: user_marker_wgsl });

  const beacon_pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: {
      module: shader,
      entryPoint: "beacon_vs_main",
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
      entryPoint: "beacon_fs_main",
      targets: [{
        format,
        blend: {
          color: {
            srcFactor: "src-alpha",
            dstFactor: "one-minus-src-alpha",
            operation: "add",
          },
          alpha: {
            srcFactor: "one",
            dstFactor: "one-minus-src-alpha",
            operation: "add",
          },
        },
      }],
    },
    primitive: {
      topology: "triangle-list",
      cullMode: "back",
    },
    depthStencil: {
      format: "depth24plus",
      depthWriteEnabled: false,
      depthCompare: "always",
    },
  });

  const zone_pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: {
      module: shader,
      entryPoint: "zone_vs_main",
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
        {
          arrayStride: 16,
          stepMode: "instance",
          attributes: [{ shaderLocation: 3, offset: 0, format: "float32x4" }],
        },
      ],
    },
    fragment: {
      module: shader,
      entryPoint: "zone_fs_main",
      targets: [{
        format,
        blend: {
          color: {
            srcFactor: "src-alpha",
            dstFactor: "one-minus-src-alpha",
            operation: "add",
          },
          alpha: {
            srcFactor: "one",
            dstFactor: "one-minus-src-alpha",
            operation: "add",
          },
        },
      }],
    },
    primitive: {
      topology: "triangle-list",
      cullMode: "back",
    },
    depthStencil: {
      format: "depth24plus",
      depthWriteEnabled: false,
      depthCompare: "always",
    },
  });

  return {
    device,
    beacon: {
      pipeline: beacon_pipeline,
      uniform: init_overlay_uniform_state(device, beacon_pipeline),
      pos_scale_buf: null,
      color_buf: null,
      instance_cnt: 0,
      instance_cap: 0,
    },
    zone: {
      pipeline: zone_pipeline,
      uniform: init_overlay_uniform_state(device, zone_pipeline),
      pos_scale_buf: null,
      color_buf: null,
      borders_buf: null,
      instance_cnt: 0,
      instance_cap: 0,
    },
  };
}

export function dispose_user_marker_pass(pass_state: UserMarkerPass): void {
  pass_state.beacon.pos_scale_buf?.destroy();
  pass_state.beacon.color_buf?.destroy();
  pass_state.beacon.pos_scale_buf = null;
  pass_state.beacon.color_buf = null;
  pass_state.beacon.instance_cnt = 0;
  pass_state.beacon.instance_cap = 0;
  pass_state.beacon.uniform.uniform_buf.destroy();

  pass_state.zone.pos_scale_buf?.destroy();
  pass_state.zone.color_buf?.destroy();
  pass_state.zone.borders_buf?.destroy();
  pass_state.zone.pos_scale_buf = null;
  pass_state.zone.color_buf = null;
  pass_state.zone.borders_buf = null;
  pass_state.zone.instance_cnt = 0;
  pass_state.zone.instance_cap = 0;
  pass_state.zone.uniform.uniform_buf.destroy();
}

export function draw_user_marker_pass(
  pass_state: UserMarkerPass,
  anchor_registry: AnchorRegistry,
  pass: GPURenderPassEncoder,
  cube_idx_cnt: u32,
  camera: RenderCameraState,
  world: RenderWorldState,
  time: f64,
  _dt: f64,
): void {
  const built = build_user_marker_instances(anchor_registry, time);
  if (!built) return;

  upload_beacon_instances(pass_state, built.beacons);
  upload_zone_instances(pass_state, built.zones);

  if (pass_state.zone.instance_cnt > 0) {
    write_overlay_uniform_state(pass_state.device, pass_state.zone.uniform, camera, world);

    pass.setPipeline(pass_state.zone.pipeline);
    pass.setBindGroup(0, pass_state.zone.uniform.bind_group);
    pass.setVertexBuffer(1, pass_state.zone.pos_scale_buf!);
    pass.setVertexBuffer(2, pass_state.zone.color_buf!);
    pass.setVertexBuffer(3, pass_state.zone.borders_buf!);
    pass.drawIndexed(cube_idx_cnt, pass_state.zone.instance_cnt, 0, 0, 0);
  }

  if (pass_state.beacon.instance_cnt > 0) {
    write_overlay_uniform_state(pass_state.device, pass_state.beacon.uniform, camera, world);

    pass.setPipeline(pass_state.beacon.pipeline);
    pass.setBindGroup(0, pass_state.beacon.uniform.bind_group);
    pass.setVertexBuffer(1, pass_state.beacon.pos_scale_buf!);
    pass.setVertexBuffer(2, pass_state.beacon.color_buf!);
    pass.drawIndexed(cube_idx_cnt, pass_state.beacon.instance_cnt, 0, 0, 0);
  }
}

function build_user_marker_instances(
  anchor_registry: AnchorRegistry,
  time: f64,
): { beacons: UserMarkerBeaconInstance[]; zones: UserMarkerZoneInstance[] } | null {
  const location = useDeviceLocationStore.getState().location;
  if (!location) return null;

  const coord = epsg4326.project({ i: 0n, f: 0n }, location.coords);

  const center_world = projection.unproject([0, 0, 0], coord.i, coord.f);
  const center_local = anchor_registry.lower_world_pos(coord.i, coord.f);
  const origin_z = ivec.to_real(ivec.Z(anchor_registry.get_map_origin()));

  center_world[2] = get_ele(coord.i, coord.f) ?? 0;
  center_local[2] = center_world[2] - origin_z;
  floor3n(center_local, U_PER_CELL, center_local);

  const center = copy3(center_local, [0, 0, 0]);

  const theme_key = get_theme_key();
  const is_night = theme_key === WorldThemeKey.NIGHT;

  const beacon_color: [f32, f32, f32, f32] = is_night ? [0.0, 0.95, 1.0, 1.0] : [1.0, 0.08, 0.58, 0.95];

  const zone_color: [f32, f32, f32, f32] = [0.31, 0.82, 0.77, 0.35];

  const beacon = center3n(
    [center[0], center[1], center[2] + BEACON_Z_BIAS + Math.sin(time * 2) * 0.2],
    U_PER_CELL,
  );

  const beacons: UserMarkerBeaconInstance[] = [{
    pos: beacon,
    scale: U_PER_CELL * 0.8,
    color: beacon_color,
  }];

  const zones: UserMarkerZoneInstance[] = [];

  for (let i: u32 = 0; i < ZONE_SAMPLES.length; i++) {
    const { off, borders } = ZONE_SAMPLES[i];

    add3(center_world, off, $0);
    const sample_coord: PVF3 = projection.project({ i: 0n, f: 0n }, $0[0], $0[1], $0[2]);

    const sample_ele: f64 = get_ele(sample_coord.i, sample_coord.f) ?? center_world[2];
    const sample_ele_local: f64 = sample_ele - origin_z;

    set3(off[0], off[1], off[2] + (sample_ele_local - center[2]), $1);
    center3n($1, U_PER_CELL, $1);
    add3(center, $1, $2);

    zones.push({
      pos: copy3($2, [0, 0, 0]),
      scale: U_PER_CELL,
      color: zone_color,
      borders: [borders[0], borders[1], borders[2], borders[3]],
    });
  }

  return { beacons, zones };
}

function ensure_beacon_cap(pass_state: UserMarkerPass, next_cnt: u32): void {
  if (next_cnt <= pass_state.beacon.instance_cap) return;

  let next_cap = Math.max(1, pass_state.beacon.instance_cap);
  while (next_cap < next_cnt) next_cap <<= 1;

  pass_state.beacon.pos_scale_buf?.destroy();
  pass_state.beacon.color_buf?.destroy();

  pass_state.beacon.pos_scale_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.beacon.color_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.beacon.instance_cap = next_cap;
  pass_state.beacon.instance_cnt = 0;
}

function upload_beacon_instances(pass_state: UserMarkerPass, instances: UserMarkerBeaconInstance[]): void {
  const next_cnt: u32 = instances.length;
  ensure_beacon_cap(pass_state, next_cnt);

  const pos_scale = new Float32Array(next_cnt * 4);
  const color = new Float32Array(next_cnt * 4);

  for (let i: u32 = 0; i < next_cnt; i++) {
    const inst = instances[i];
    const off = i * 4;

    pos_scale[off + 0] = inst.pos[0];
    pos_scale[off + 1] = inst.pos[1];
    pos_scale[off + 2] = inst.pos[2];
    pos_scale[off + 3] = inst.scale;

    color[off + 0] = inst.color[0];
    color[off + 1] = inst.color[1];
    color[off + 2] = inst.color[2];
    color[off + 3] = inst.color[3];
  }

  pass_state.device.queue.writeBuffer(pass_state.beacon.pos_scale_buf!, 0, pos_scale);
  pass_state.device.queue.writeBuffer(pass_state.beacon.color_buf!, 0, color);
  pass_state.beacon.instance_cnt = next_cnt;
}

function ensure_zone_cap(pass_state: UserMarkerPass, next_cnt: u32): void {
  if (next_cnt <= pass_state.zone.instance_cap) return;

  let next_cap = Math.max(1, pass_state.zone.instance_cap);
  while (next_cap < next_cnt) next_cap <<= 1;

  pass_state.zone.pos_scale_buf?.destroy();
  pass_state.zone.color_buf?.destroy();
  pass_state.zone.borders_buf?.destroy();

  pass_state.zone.pos_scale_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.zone.color_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.zone.borders_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.zone.instance_cap = next_cap;
  pass_state.zone.instance_cnt = 0;
}

function upload_zone_instances(pass_state: UserMarkerPass, instances: UserMarkerZoneInstance[]): void {
  const next_cnt: u32 = instances.length;
  ensure_zone_cap(pass_state, next_cnt);

  const pos_scale = new Float32Array(next_cnt * 4);
  const color = new Float32Array(next_cnt * 4);
  const borders = new Float32Array(next_cnt * 4);

  for (let i: u32 = 0; i < next_cnt; i++) {
    const inst = instances[i];
    const off = i * 4;

    pos_scale[off + 0] = inst.pos[0];
    pos_scale[off + 1] = inst.pos[1];
    pos_scale[off + 2] = inst.pos[2];
    pos_scale[off + 3] = inst.scale;

    color[off + 0] = inst.color[0];
    color[off + 1] = inst.color[1];
    color[off + 2] = inst.color[2];
    color[off + 3] = inst.color[3];

    borders[off + 0] = inst.borders[0];
    borders[off + 1] = inst.borders[1];
    borders[off + 2] = inst.borders[2];
    borders[off + 3] = inst.borders[3];
  }

  pass_state.device.queue.writeBuffer(pass_state.zone.pos_scale_buf!, 0, pos_scale);
  pass_state.device.queue.writeBuffer(pass_state.zone.color_buf!, 0, color);
  pass_state.device.queue.writeBuffer(pass_state.zone.borders_buf!, 0, borders);
  pass_state.zone.instance_cnt = next_cnt;
}
