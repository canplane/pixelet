// @pxl/web/render/overlay/types.ts

import type { F32Array } from "@xpute/spec/lib/kit/array.ts";
import type { f32 } from "@xpute/spec/lib/kit/word.ts";

import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";

export type OverlayUniformState = {
  uniform_buf: GPUBuffer;
  bind_group: GPUBindGroup;
  uniform_f32: F32Array;
};

// export type OverlayCubeGeometry = {
//   cube_vertex_buf: GPUBuffer;
//   cube_idx_buf: GPUBuffer;
//   cube_idx_cnt: u32;
// };

export type OverlayColor = [f32, f32, f32, f32];

export function init_overlay_uniform_state(
  device: GPUDevice,
  pipeline: GPURenderPipeline,
): OverlayUniformState {
  const uniform_f32 = new Float32Array(28);
  const uniform_buf = device.createBuffer({
    size: uniform_f32.byteLength,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });

  const bind_group = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniform_buf } }],
  });

  return {
    uniform_buf,
    bind_group,
    uniform_f32,
  };
}

export function write_overlay_uniform_state(
  device: GPUDevice,
  state: OverlayUniformState,
  camera: RenderCameraState,
  world: RenderWorldState,
): void {
  state.uniform_f32.set(camera.view_proj, 0);

  state.uniform_f32[16] = world.world_azimuth;
  state.uniform_f32[17] = 0;
  state.uniform_f32[18] = 0;
  state.uniform_f32[19] = 0;

  state.uniform_f32[20] = world.local_world_offset[0];
  state.uniform_f32[21] = world.local_world_offset[1];
  state.uniform_f32[22] = world.local_world_offset[2];
  state.uniform_f32[23] = 0;

  state.uniform_f32[24] = 0;
  state.uniform_f32[25] = 0;
  state.uniform_f32[26] = 0;
  state.uniform_f32[27] = 0;

  device.queue.writeBuffer(state.uniform_buf, 0, state.uniform_f32);
}
