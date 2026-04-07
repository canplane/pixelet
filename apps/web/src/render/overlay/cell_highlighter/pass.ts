// @pxl/web/render/overlay/cell_highlighter/pass.ts

import type { u32 } from "@xpute/spec/lib/kit/word.ts";

import { center3n, floor3n } from "@pxl/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import type { AnchorRegistry } from "@/render/anchor/registry.ts";
import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";
import { init_overlay_uniform_state, type OverlayUniformState, write_overlay_uniform_state } from "@/render/overlay/types.ts";

import type { CellHighlighterInstance } from "./types.ts";
import cell_highlighter_wgsl from "./cell_highlighter.wgsl?raw";

export type CellHighlighterPass = {
  device: GPUDevice;

  pipeline: GPURenderPipeline;
  uniform: OverlayUniformState;

  instance_pos_scale_buf: GPUBuffer | null;
  instance_color_buf: GPUBuffer | null;

  instance_cnt: u32;
  instance_cap: u32;
};

export function init_cell_highlighter_pass(device: GPUDevice, format: GPUTextureFormat): CellHighlighterPass {
  const shader = device.createShaderModule({ code: cell_highlighter_wgsl });

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
    pipeline,
    uniform: init_overlay_uniform_state(device, pipeline),
    instance_pos_scale_buf: null,
    instance_color_buf: null,
    instance_cnt: 0,
    instance_cap: 0,
  };
}

export function dispose_cell_highlighter_pass(pass_state: CellHighlighterPass): void {
  pass_state.instance_pos_scale_buf?.destroy();
  pass_state.instance_color_buf?.destroy();
  pass_state.instance_pos_scale_buf = null;
  pass_state.instance_color_buf = null;
  pass_state.instance_cnt = 0;
  pass_state.instance_cap = 0;
  pass_state.uniform.uniform_buf.destroy();
}

export function draw_cell_highlighter_pass(
  pass_state: CellHighlighterPass,
  anchor_registry: AnchorRegistry,
  pass: GPURenderPassEncoder,
  cube_idx_cnt: u32,
  camera: RenderCameraState,
  world: RenderWorldState,
): void {
  const instances = build_cell_highlighter_instances(anchor_registry);
  if (instances.length === 0) return;

  upload_instances(pass_state, instances);
  write_overlay_uniform_state(pass_state.device, pass_state.uniform, camera, world);

  pass.setPipeline(pass_state.pipeline);
  pass.setBindGroup(0, pass_state.uniform.bind_group);
  pass.setVertexBuffer(1, pass_state.instance_pos_scale_buf!);
  pass.setVertexBuffer(2, pass_state.instance_color_buf!);
  pass.drawIndexed(cube_idx_cnt, pass_state.instance_cnt, 0, 0, 0);
}

function build_cell_highlighter_instances(anchor_registry: AnchorRegistry): CellHighlighterInstance[] {
  const hovered = useCameraStore.getState().hovered_coord;
  if (!hovered) return [];

  const pos = anchor_registry.lower_world_pos(hovered.i, hovered.f);
  floor3n(pos, U_PER_CELL, pos);

  return [{
    pos: center3n([pos[0], pos[1], pos[2] - U_PER_CELL], U_PER_CELL),
    scale: U_PER_CELL,
    color: [1.0, 0.62, 0.02, 0.60],
  }];
}

function ensure_instance_cap(pass_state: CellHighlighterPass, next_cnt: u32): void {
  if (next_cnt <= pass_state.instance_cap) return;

  let next_cap = Math.max(1, pass_state.instance_cap);
  while (next_cap < next_cnt) next_cap <<= 1;

  pass_state.instance_pos_scale_buf?.destroy();
  pass_state.instance_color_buf?.destroy();

  pass_state.instance_pos_scale_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.instance_color_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.instance_cap = next_cap;
  pass_state.instance_cnt = 0;
}

function upload_instances(pass_state: CellHighlighterPass, instances: CellHighlighterInstance[]): void {
  const next_cnt: u32 = instances.length;
  ensure_instance_cap(pass_state, next_cnt);

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

  pass_state.device.queue.writeBuffer(pass_state.instance_pos_scale_buf!, 0, pos_scale);
  pass_state.device.queue.writeBuffer(pass_state.instance_color_buf!, 0, color);
  pass_state.instance_cnt = next_cnt;
}
