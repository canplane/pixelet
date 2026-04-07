// @pxl/web/render/runtime.ts

import type { RenderCameraState, RenderSurface, RenderWorldState } from "./types.ts";

let booted = false;
let gpu_device: GPUDevice | null = null;
let gpu_adapter: GPUAdapter | null = null;

let render_surface: RenderSurface | null = null;
let render_camera_state: RenderCameraState | null = null;
let render_world_state: RenderWorldState | null = null;

export async function init_render_runtime(): Promise<void> {
  if (booted) return;

  if (!navigator.gpu) {
    throw new Error("web: WebGPU unsupported");
  }

  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: "high-performance",
  });
  if (!adapter) {
    throw new Error("web: failed to acquire WebGPU adapter");
  }

  const device = await adapter.requestDevice();

  gpu_adapter = adapter;
  gpu_device = device;
  booted = true;
}

export function get_gpu_device(): GPUDevice {
  if (!gpu_device) {
    throw new Error("web: render runtime not initialized");
  }
  return gpu_device;
}

export function get_gpu_adapter(): GPUAdapter {
  if (!gpu_adapter) {
    throw new Error("web: render runtime not initialized");
  }
  return gpu_adapter;
}

export function set_render_surface(x: RenderSurface | null): void {
  render_surface = x;
}

export function get_render_surface(): RenderSurface | null {
  return render_surface;
}

export function set_render_camera_state(x: RenderCameraState | null): void {
  render_camera_state = x;
}

export function get_render_camera_state(): RenderCameraState | null {
  return render_camera_state;
}

export function set_render_world_state(x: RenderWorldState | null): void {
  render_world_state = x;
}

export function get_render_world_state(): RenderWorldState | null {
  return render_world_state;
}
