// @pxl/web/render/overlay/pass.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";
import { type IVec, ivec, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L3_BASE, L3_CENTER } from "@pxl/spec/space/indexer.ts";
import { TO_XY } from "@pxl/spec/space/xy.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { get_ele } from "@/modules/world/terrain/use-elevation.ts";
import { create_static_buffer } from "@/render/common/gpu_buffer.ts";
import { CUBE_INDICES, CUBE_VERTICES } from "@/render/common/cube.ts";
import { SPHERE_INDICES, SPHERE_VERTICES } from "@/render/primitives/sphere/mesh.ts";
import type { AnchorRegistry } from "@/render/anchor/registry.ts";
import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";

import { type CellHighlighterPass, dispose_cell_highlighter_pass, draw_cell_highlighter_pass, init_cell_highlighter_pass } from "./cell_highlighter/pass.ts";
import { dispose_user_marker_pass, draw_user_marker_pass, init_user_marker_pass, type UserMarkerPass } from "./user_marker/pass.ts";
import { dispose_block_pass, draw_block_pass, init_block_pass } from "@/render/primitives/block/pass.ts";
import { dispose_sphere_pass, draw_sphere_pass, init_sphere_pass } from "@/render/primitives/sphere/pass.ts";
import type { BlockInstance } from "@/render/primitives/block/types.ts";
import type { SphereInstance, SpherePass } from "@/render/primitives/sphere/types.ts";
import type { BlockPass } from "@/render/primitives/block/types.ts";

export type OverlayPass = {
  device: GPUDevice;

  cube_vertex_buf: GPUBuffer;
  cube_idx_buf: GPUBuffer;
  cube_idx_cnt: u32;

  sphere_vertex_buf: GPUBuffer;
  sphere_idx_buf: GPUBuffer;
  sphere_idx_cnt: u32;

  block_pass: BlockPass;
  sphere_pass: SpherePass;
  cell_highlighter_pass: CellHighlighterPass;
  user_marker_pass: UserMarkerPass;
};

export function init_overlay_pass(device: GPUDevice, format: GPUTextureFormat): OverlayPass {
  const cube_vertex_buf = create_static_buffer(device, GPUBufferUsage.VERTEX, CUBE_VERTICES);
  const cube_idx_buf = create_static_buffer(device, GPUBufferUsage.INDEX, CUBE_INDICES);
  const sphere_vertex_buf = create_static_buffer(device, GPUBufferUsage.VERTEX, SPHERE_VERTICES);
  const sphere_idx_buf = create_static_buffer(device, GPUBufferUsage.INDEX, SPHERE_INDICES);

  return {
    device,
    cube_vertex_buf,
    cube_idx_buf,
    cube_idx_cnt: CUBE_INDICES.length,
    sphere_vertex_buf,
    sphere_idx_buf,
    sphere_idx_cnt: SPHERE_INDICES.length,
    block_pass: init_block_pass(device, format),
    sphere_pass: init_sphere_pass(device, format),
    cell_highlighter_pass: init_cell_highlighter_pass(device, format),
    user_marker_pass: init_user_marker_pass(device, format),
  };
}

export function dispose_overlay_pass(pass_state: OverlayPass): void {
  dispose_block_pass(pass_state.block_pass);
  dispose_sphere_pass(pass_state.sphere_pass);
  dispose_cell_highlighter_pass(pass_state.cell_highlighter_pass);
  dispose_user_marker_pass(pass_state.user_marker_pass);

  pass_state.cube_vertex_buf.destroy();
  pass_state.cube_idx_buf.destroy();
  pass_state.sphere_vertex_buf.destroy();
  pass_state.sphere_idx_buf.destroy();
}

export function draw_overlay_pass(
  pass_state: OverlayPass,
  anchor_registry: AnchorRegistry,
  pass: GPURenderPassEncoder,
  camera: RenderCameraState,
  world: RenderWorldState,
  time: f64,
  dt: f64,
): void {
  pass.setVertexBuffer(0, pass_state.cube_vertex_buf);
  pass.setIndexBuffer(pass_state.cube_idx_buf, "uint16");

  if (__IS_DEV__) {
    const map_center: PVF3 | null = useMapStore.getState().map_center;
    if (map_center !== null) {
      const map_origin: IVec = anchor_registry.get_map_origin();
      const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));

      const spheres: SphereInstance[] = [
        { center: [5, 0, 0], radius: 1, color: [0, 1, 0, 1] },
        { center: [0, 5, 0], radius: 1, color: [1, 1, 0, 1] },
        { center: [2048, 2048, 0], radius: 100, color: [0, 0, 0, 1] },
      ];

      {
        const center = anchor_registry.lower_world_pos(map_center.i, map_center.f);
        const ele = get_ele(map_center.i, map_center.f);
        if (ele !== null) center[2] = ele - origin_z;
        spheres.push({
          center,
          radius: 0.5,
          color: [1, 0, 0, 1],
        });
      }

      {
        const center = anchor_registry.lower_world_pos(map_origin, 0n);
        const ele = get_ele(map_origin, 0n);
        if (ele !== null) center[2] = ele - origin_z;
        spheres.push({
          center,
          radius: 0.5,
          color: [0, 0, 1, 1],
        });
      }

      const blocks: BlockInstance[] = [];

      {
        const i: IVec = TO_XY(L3_BASE(map_center.i));
        const lowered = anchor_registry.lower_anchor_local_coord(i, 0n);
        const anchor_base = anchor_registry.get_anchor_local_base(lowered.anchor_idx);

        blocks.push({
          base: [
            anchor_base[0] + lowered.anchor_local_pos[0] + 0,
            anchor_base[1] + lowered.anchor_local_pos[1] + U_PER_CELL,
            (get_ele(i, 0n) ?? 0) - origin_z + 0,
          ],
          span: 1,
          color: [0, 1, 0, 1],
        });
      }

      {
        const i: IVec = TO_XY(L3_CENTER(map_center.i));
        const lowered = anchor_registry.lower_anchor_local_coord(i, 0n);
        const anchor_base = anchor_registry.get_anchor_local_base(lowered.anchor_idx);

        blocks.push({
          base: [
            anchor_base[0] + lowered.anchor_local_pos[0] + U_PER_CELL,
            anchor_base[1] + lowered.anchor_local_pos[1] + 0,
            (get_ele(i, 0n) ?? 0) - origin_z + 0,
          ],
          span: 1,
          color: [1, 0, 0, 1],
        });
      }

      {
        const lowered = anchor_registry.lower_anchor_local_coord(map_center.i, map_center.f);
        const anchor_base = anchor_registry.get_anchor_local_base(lowered.anchor_idx);

        blocks.push({
          base: [
            anchor_base[0] + lowered.anchor_local_pos[0] + 0,
            anchor_base[1] + lowered.anchor_local_pos[1] + 0,
            anchor_base[2] + lowered.anchor_local_pos[2] + 0,
          ],
          span: 1,
          color: [0, 0, 1, 1],
        });
      }

      draw_block_pass(
        pass_state.block_pass,
        pass,
        pass_state.cube_idx_cnt,
        camera,
        world,
        blocks,
      );

      pass.setVertexBuffer(0, pass_state.sphere_vertex_buf);
      pass.setIndexBuffer(pass_state.sphere_idx_buf, "uint16");

      draw_sphere_pass(
        pass_state.sphere_pass,
        pass,
        pass_state.sphere_idx_cnt,
        camera,
        world,
        spheres,
      );

      pass.setVertexBuffer(0, pass_state.cube_vertex_buf);
      pass.setIndexBuffer(pass_state.cube_idx_buf, "uint16");
    }
  }

  draw_cell_highlighter_pass(pass_state.cell_highlighter_pass, anchor_registry, pass, pass_state.cube_idx_cnt, camera, world);
  draw_user_marker_pass(pass_state.user_marker_pass, anchor_registry, pass, pass_state.cube_idx_cnt, camera, world, time, dt);
}
