// @pxl/web/render/gpu/stage.tsx

/**
 * The WebGPU canvas beside three's, and the frame's uniforms read off
 * what three still owns — the camera, the lights, the fog — until it goes.
 * The canvas sits over three's, cleared transparent and passing pointer
 * events through, so what it draws shows on top of what three draws; the
 * order flips once the sky and everything under it have moved.
 */

import { useEffect, useRef } from "react";
import * as THREE from "three";
import type { RootState } from "@react-three/fiber";

import { useSettingsStore } from "@/app/setting/store.ts";
import { light_cfg } from "@/module/world/environment/config.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { GpuRenderer } from "./renderer.ts";

/** The one renderer, once its device has answered. */
export let gpu_renderer: GpuRenderer | null = null;

/** Whether the chunks are drawn here rather than by three. */
export function gpu_draws_chunks(): boolean {
  return gpu_renderer !== null && useSettingsStore.getState().gpu_chunks;
}

const EDGE_STRENGTH = 0.3;
const EDGE_WIDTH = 0.03;

const $m = new THREE.Matrix4();
/** GL clip z is [-1, 1]; WebGPU's is [0, 1]. */
const $gl_to_wgpu = new THREE.Matrix4().set(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0.5, 0.5, 0, 0, 0, 1);
const $v = new THREE.Vector3();

type Lights = { sun: THREE.DirectionalLight | null; ambient: THREE.AmbientLight | null; hemi: THREE.HemisphereLight | null; seen: number };
const lights: Lights = { sun: null, ambient: null, hemi: null, seen: -1 };

/** The scene's lights, looked up when the scene's children change. */
function find_lights(scene: THREE.Scene): Lights {
  if (lights.seen === scene.children.length) return lights;
  lights.seen = scene.children.length;
  lights.sun = null;
  lights.ambient = null;
  lights.hemi = null;
  for (const o of scene.children) {
    if (o instanceof THREE.DirectionalLight && (lights.sun === null || o.intensity > lights.sun.intensity)) lights.sun = o;
    else if (o instanceof THREE.AmbientLight) lights.ambient = o;
    else if (o instanceof THREE.HemisphereLight) lights.hemi = o;
  }
  return lights;
}

function put(u: Float32Array, at: number, c: THREE.Color, k: number): void {
  u[at] = c.r * k;
  u[at + 1] = c.g * k;
  u[at + 2] = c.b * k;
}

/**
 * Writes the frame's uniforms and draws: called from the chunk stage's
 * visible step after the module's tick, with the anchor root the chunks'
 * translations are relative to.
 */
export function gpu_frame(state: RootState, root: THREE.Matrix4): void {
  const gpu = gpu_renderer;
  if (!gpu) return;
  const dpr = state.viewport.dpr;
  gpu.resize(state.size.width * dpr, state.size.height * dpr);
  const cam = state.camera as THREE.PerspectiveCamera;
  cam.updateMatrixWorld();
  $m.copy($gl_to_wgpu).multiply(cam.projectionMatrix).multiply(cam.matrixWorldInverse).multiply(root);
  const u = gpu.uniforms;
  u.set($m.elements, 0);
  // The camera's position in the root's frame, where the meshes are.
  $v.copy(cam.position).applyMatrix4($m.copy(root).invert());
  u[16] = $v.x;
  u[17] = $v.y;
  u[18] = $v.z;
  const settings = useSettingsStore.getState();
  u[19] = settings.show_edge_shade ? EDGE_STRENGTH : 0;
  const l = find_lights(state.scene);
  if (l.sun) {
    // The light's direction in the root's frame: the world direction, the
    // root's rotation undone.
    $v.copy(l.sun.position).sub(l.sun.target.position).normalize().transformDirection($m.copy(root).invert());
    u[20] = $v.x;
    u[21] = $v.y;
    u[22] = $v.z;
    put(u, 24, l.sun.color, l.sun.intensity);
  } else {
    u[20] = 0;
    u[21] = 0;
    u[22] = 1;
    u[24] = u[25] = u[26] = 0;
  }
  u[23] = settings.texture_pack ? 0 : 1;
  const fog = state.scene.fog instanceof THREE.Fog ? state.scene.fog : null;
  u[27] = fog ? fog.near : 1e9;
  u[31] = fog ? fog.far : 2e9;
  if (l.ambient) put(u, 28, l.ambient.color, l.ambient.intensity);
  else u[28] = u[29] = u[30] = 0;
  if (l.hemi) {
    put(u, 32, l.hemi.color, l.hemi.intensity);
    put(u, 36, l.hemi.groundColor, l.hemi.intensity);
  } else {
    u[32] = u[33] = u[34] = 0;
    u[36] = u[37] = u[38] = 0;
  }
  u[35] = light_cfg.FLOOR;
  u[39] = EDGE_WIDTH;
  if (fog) put(u, 40, fog.color, 1);
  else u[40] = u[41] = u[42] = 0;
  gpu.frame();
}

/** The canvas over three's; the renderer once the device answers. */
export function GpuCanvas() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || CORE_PXL === null) return;
    let alive = true;
    GpuRenderer.create(canvas, CORE_PXL).then((r) => {
      if (!alive) return;
      if (r === null) {
        console.error("[gpu] no WebGPU on this device; the chunks stay three's");
        return;
      }
      gpu_renderer = r;
    });
    return () => {
      alive = false;
      gpu_renderer = null;
    };
  }, []);
  return <canvas ref={ref} className="absolute inset-0 h-full w-full" style={{ pointerEvents: "none" }} />;
}
