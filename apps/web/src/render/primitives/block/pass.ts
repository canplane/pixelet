// @pxl/web/render/primitives/block/pass.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { center3, fill3, type V3 } from "@pxl/spec/space/v3.ts";

import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";
import { init_overlay_uniform_state, write_overlay_uniform_state } from "@/render/overlay/types.ts";

import type { BlockInstance, BlockPass } from "./types.ts";
import block_wgsl from "./block.wgsl?raw";

function lower_block(inst: BlockInstance): { center: V3; span: V3 } {
  const span: V3 = typeof inst.span === "number" ? fill3(inst.span) : inst.span;

  return {
    center: center3(inst.base, span),
    span,
  };
}

export function init_block_pass(device: GPUDevice, format: GPUTextureFormat): BlockPass {
  const shader = device.createShaderModule({ code: block_wgsl });

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
        {
          arrayStride: 16,
          stepMode: "instance",
          attributes: [{ shaderLocation: 3, offset: 0, format: "float32x4" }],
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
      depthCompare: "less-equal",
    },
  });

  return {
    device,
    pipeline,
    uniform: init_overlay_uniform_state(device, pipeline),
    instance_center_buf: null,
    instance_span_buf: null,
    instance_color_buf: null,
    instance_cnt: 0,
    instance_cap: 0,
  };
}

export function dispose_block_pass(pass_state: BlockPass): void {
  pass_state.instance_center_buf?.destroy();
  pass_state.instance_span_buf?.destroy();
  pass_state.instance_color_buf?.destroy();

  pass_state.instance_center_buf = null;
  pass_state.instance_span_buf = null;
  pass_state.instance_color_buf = null;
  pass_state.instance_cnt = 0;
  pass_state.instance_cap = 0;

  pass_state.uniform.uniform_buf.destroy();
}

function ensure_instance_cap(pass_state: BlockPass, next_cnt: u32): void {
  if (next_cnt <= pass_state.instance_cap) return;

  let next_cap = Math.max(1, pass_state.instance_cap);
  while (next_cap < next_cnt) next_cap <<= 1;

  pass_state.instance_center_buf?.destroy();
  pass_state.instance_span_buf?.destroy();
  pass_state.instance_color_buf?.destroy();

  pass_state.instance_center_buf = pass_state.device.createBuffer({
    size: next_cap * 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  pass_state.instance_span_buf = pass_state.device.createBuffer({
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

function upload_instances(pass_state: BlockPass, instances: BlockInstance[]): void {
  const next_cnt: u32 = instances.length;
  ensure_instance_cap(pass_state, next_cnt);

  const center = new Float32Array(next_cnt * 4);
  const span = new Float32Array(next_cnt * 4);
  const color = new Float32Array(next_cnt * 4);

  for (let i: u32 = 0; i < next_cnt; i++) {
    const inst = instances[i];
    const off = i * 4;

    const lowered = lower_block(inst);

    center[off + 0] = lowered.center[0];
    center[off + 1] = lowered.center[1];
    center[off + 2] = lowered.center[2];
    center[off + 3] = 0;

    span[off + 0] = lowered.span[0];
    span[off + 1] = lowered.span[1];
    span[off + 2] = lowered.span[2];
    span[off + 3] = 0;

    color[off + 0] = inst.color[0];
    color[off + 1] = inst.color[1];
    color[off + 2] = inst.color[2];
    color[off + 3] = inst.color[3];
  }

  pass_state.device.queue.writeBuffer(pass_state.instance_center_buf!, 0, center);
  pass_state.device.queue.writeBuffer(pass_state.instance_span_buf!, 0, span);
  pass_state.device.queue.writeBuffer(pass_state.instance_color_buf!, 0, color);
  pass_state.instance_cnt = next_cnt;
}

export function draw_block_pass(
  pass_state: BlockPass,
  pass: GPURenderPassEncoder,
  cube_idx_cnt: u32,
  camera: RenderCameraState,
  world: RenderWorldState,
  instances: BlockInstance[],
): void {
  if (instances.length === 0) return;

  upload_instances(pass_state, instances);
  write_overlay_uniform_state(pass_state.device, pass_state.uniform, camera, world);

  pass.setPipeline(pass_state.pipeline);
  pass.setBindGroup(0, pass_state.uniform.bind_group);
  pass.setVertexBuffer(1, pass_state.instance_center_buf!);
  pass.setVertexBuffer(2, pass_state.instance_span_buf!);
  pass.setVertexBuffer(3, pass_state.instance_color_buf!);
  pass.drawIndexed(cube_idx_cnt, pass_state.instance_cnt, 0, 0, 0);
}
