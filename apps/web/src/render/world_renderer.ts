// @pxl/web/render/world_renderer.ts

import type { u32 } from "@xpute/spec/lib/kit/word.ts";

import type { RenderFrameArgs } from "./types.ts";
import { get_render_camera_state, get_render_world_state } from "./runtime.ts";

import { get_theme_key, themes, type WorldTheme } from "@/modules/world/render/themes.ts";

import { AnchorRegistry } from "@/render/anchor/registry.ts";
import { dispose_overlay_pass, draw_overlay_pass, init_overlay_pass, type OverlayPass } from "@/render/overlay/pass.ts";

export type WorldRenderer = {
  device: GPUDevice;
  format: GPUTextureFormat;

  anchor_registry: AnchorRegistry;
  overlay_pass: OverlayPass;

  depth_tex: GPUTexture | null;
  depth_view: GPUTextureView | null;
  depth_width: u32;
  depth_height: u32;
};

export function init_world_renderer(device: GPUDevice, format: GPUTextureFormat): WorldRenderer {
  return {
    device,
    format,
    anchor_registry: new AnchorRegistry(),
    overlay_pass: init_overlay_pass(device, format),
    depth_tex: null,
    depth_view: null,
    depth_width: 0,
    depth_height: 0,
  };
}

export function dispose_world_renderer(renderer: WorldRenderer): void {
  dispose_overlay_pass(renderer.overlay_pass);
  renderer.anchor_registry.clear();

  renderer.depth_view = null;
  renderer.depth_tex?.destroy();
  renderer.depth_tex = null;
  renderer.depth_width = 0;
  renderer.depth_height = 0;
}

export function draw_world_renderer(renderer: WorldRenderer, args: RenderFrameArgs): void {
  const { surface, time, dt } = args;

  const camera = get_render_camera_state();
  const world = get_render_world_state();
  if (!camera || !world) return;

  renderer.anchor_registry.set_map_origin(world.map_origin);

  ensure_depth(renderer, surface.width, surface.height);

  const theme_key = get_theme_key();
  const theme: WorldTheme = themes[theme_key];

  const view = surface.context.getCurrentTexture().createView();
  const fog = hex_to_rgba(theme.fog);

  const encoder = renderer.device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [{
      view,
      clearValue: fog,
      loadOp: "clear",
      storeOp: "store",
    }],
    depthStencilAttachment: {
      view: renderer.depth_view!,
      depthClearValue: 1,
      depthLoadOp: "clear",
      depthStoreOp: "store",
    },
  });

  draw_overlay_pass(
    renderer.overlay_pass,
    renderer.anchor_registry,
    pass,
    camera,
    world,
    time,
    dt,
  );

  pass.end();
  renderer.device.queue.submit([encoder.finish()]);
}

function ensure_depth(renderer: WorldRenderer, width: u32, height: u32): void {
  if (
    renderer.depth_tex &&
    renderer.depth_view &&
    renderer.depth_width === width &&
    renderer.depth_height === height
  ) {
    return;
  }

  renderer.depth_view = null;
  renderer.depth_tex?.destroy();

  renderer.depth_tex = renderer.device.createTexture({
    size: {
      width: Math.max(1, width),
      height: Math.max(1, height),
    },
    format: "depth24plus",
    usage: GPUTextureUsage.RENDER_ATTACHMENT,
  });
  renderer.depth_view = renderer.depth_tex.createView();
  renderer.depth_width = width;
  renderer.depth_height = height;
}

function hex_to_rgba(hex: string): GPUColor {
  const x = hex.startsWith("#") ? hex.slice(1) : hex;

  if (x.length === 3) {
    const r = parseInt(x[0] + x[0], 16) / 255;
    const g = parseInt(x[1] + x[1], 16) / 255;
    const b = parseInt(x[2] + x[2], 16) / 255;
    return { r, g, b, a: 1 };
  }

  const r = parseInt(x.slice(0, 2), 16) / 255;
  const g = parseInt(x.slice(2, 4), 16) / 255;
  const b = parseInt(x.slice(4, 6), 16) / 255;
  return { r, g, b, a: 1 };
}
