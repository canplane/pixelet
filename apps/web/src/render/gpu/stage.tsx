// @pxl/web/render/gpu/stage.tsx

/**
 * The WebGPU canvas, and the frame's uniforms: the camera read off what
 * three still owns until it goes, the environment from the module. Everything is drawn here; three's canvas above it,
 * cleared transparent, draws only the dev gizmos.
 */

import { useEffect, useRef } from "react";
import * as THREE from "three";
import type { RootState } from "@react-three/fiber";

import { useSettingsStore } from "@/app/setting/store.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { fade_end_u, fade_start_u, LOD_CELL_PER_DISTANCE } from "@/render/lod.ts";
import { billboard_sheet } from "@/module/world/billboard/sheet.ts";
import { decor_bundle_assets, type DecorBundleAssets } from "@/module/world/decor/asset_store.ts";
import type { DecorModel } from "./decor.ts";
import { GpuRenderer } from "./renderer.ts";

/** The one renderer, once its device has answered. */
export let gpu_renderer: GpuRenderer | null = null;

/** The sky dome's radius in environment.tsx, which the sky's rays meet. */
let sky_radius = 1;
export function set_sky_radius(r: number): void {
  sky_radius = r;
}

const EDGE_STRENGTH = 0.3;
const EDGE_WIDTH = 0.03;
/** crates/pxl-core/src/environment.rs's ENV_FLOATS. */
const ENV_FLOATS = 48;

const $m = new THREE.Matrix4();
/** GL clip z is [-1, 1]; WebGPU's is [0, 1]. */
const $gl_to_wgpu = new THREE.Matrix4().set(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0.5, 0.5, 0, 0, 0, 1);
const $v = new THREE.Vector3();
const $sky = new THREE.Matrix4();

/** decor/asset_store.ts's models and skins by group, as the renderer wants
 * them, rebuilt when the assets change. */
let decor_assets_seen: readonly DecorBundleAssets[] | null = null;
const decor_models = new Map<THREE.BufferGeometry, Omit<DecorModel, "skin">>();

function decor_model(geometry: THREE.BufferGeometry | null, material: THREE.MeshStandardMaterial | null): DecorModel | null {
  const skin = material?.map?.image as DecorModel["skin"] | undefined;
  if (!geometry || !skin) return null;
  let lanes = decor_models.get(geometry);
  if (!lanes) {
    const p = geometry.getAttribute("position");
    const nrm = geometry.getAttribute("normal");
    const uv = geometry.getAttribute("uv");
    const count = p.count;
    const position = new Float32Array(count * 3);
    const normal = new Float32Array(count * 3);
    const uvs = new Float32Array(count * 2);
    for (let i = 0; i < count; i++) {
      position[i * 3] = p.getX(i);
      position[i * 3 + 1] = p.getY(i);
      position[i * 3 + 2] = p.getZ(i);
      if (nrm) {
        normal[i * 3] = nrm.getX(i);
        normal[i * 3 + 1] = nrm.getY(i);
        normal[i * 3 + 2] = nrm.getZ(i);
      }
      if (uv) {
        uvs[i * 2] = uv.getX(i);
        uvs[i * 2 + 1] = uv.getY(i);
      }
    }
    const idx = geometry.getIndex();
    const index = new Uint32Array(idx ? idx.count : count);
    for (let i = 0; i < index.length; i++) index[i] = idx ? idx.getX(i) : i;
    lanes = { position, normal, uv: uvs, index };
    decor_models.set(geometry, lanes);
  }
  return { ...lanes, skin };
}

function sync_decor_models(gpu: GpuRenderer): void {
  const assets = decor_bundle_assets();
  if (assets === decor_assets_seen) return;
  decor_assets_seen = assets;
  const models: (DecorModel | null)[] = [];
  const by_skin = new Map<unknown, DecorModel | null>();
  for (const { geometries, materials } of assets) {
    geometries.forEach((geometry, v) => {
      const key = geometry && materials[v] ? `${geometry.uuid}:${materials[v]!.uuid}` : null;
      if (key !== null && by_skin.has(key)) {
        models.push(by_skin.get(key)!);
        return;
      }
      const m = decor_model(geometry, materials[v]);
      if (key !== null) by_skin.set(key, m);
      models.push(m);
    });
  }
  gpu.decor.set_models(models);
}

/**
 * Writes the frame's uniforms and draws: called from the chunk stage's
 * visible step after the module's tick, with the anchor root the chunks'
 * translations are relative to.
 */
export function gpu_frame(state: RootState, root: THREE.Matrix4): void {
  const gpu = gpu_renderer;
  if (!gpu) return;
  sync_decor_models(gpu);
  gpu.badges.sync_sheet(billboard_sheet);
  const dpr = state.viewport.dpr;
  gpu.resize(state.size.width * dpr, state.size.height * dpr);
  const cam = state.camera as THREE.PerspectiveCamera;
  cam.updateMatrixWorld();
  // The badges take the view and the projection apart: their size is
  // measured in view space (billboard/field.ts).
  const badges = gpu.badges;
  badges.view.set($m.copy(cam.matrixWorldInverse).multiply(root).elements);
  badges.proj.set($m.copy($gl_to_wgpu).multiply(cam.projectionMatrix).elements);
  badges.px_k = state.size.height / (2 * Math.tan((cam.fov * Math.PI) / 360));
  badges.fade_start = fade_start_u();
  badges.fade_end = fade_end_u();
  badges.near = cam.near;
  badges.far = cam.far;
  badges.cell = LOD_CELL_PER_DISTANCE.value;
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
  // The light, the fog and the sky's colours and directions are the
  // module's (crates/pxl-core/src/environment.rs), laid out as these
  // uniforms take them from their twentieth float.
  const pxl = CORE_PXL!;
  const env = new Float32Array(pxl.memory.buffer, pxl.pxl_env_at(), ENV_FLOATS);
  u.set(env.subarray(0, 24), 20);
  u[23] = settings.texture_pack ? 0 : 1;
  u[39] = EDGE_WIDTH;

  // The sky, in three's world frame, where the dome stood.
  const sky = gpu.sky;
  $sky.copy($gl_to_wgpu).multiply(cam.projectionMatrix).multiply(cam.matrixWorldInverse).invert();
  sky.set($sky.elements, 0);
  $v.setFromMatrixPosition(cam.matrixWorld);
  sky[16] = $v.x;
  sky[17] = $v.y;
  sky[18] = $v.z;
  sky[19] = sky_radius;
  sky.set(env.subarray(24, 48), 20);
  gpu.frame();
}

/** The canvas under three's; the renderer once the device answers. */
export function GpuCanvas() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || CORE_PXL === null) return;
    let alive = true;
    GpuRenderer.create(canvas, CORE_PXL).then((r) => {
      if (!alive) return;
      if (r === null) {
        console.error("[gpu] no WebGPU on this device; nothing can be drawn");
        return;
      }
      gpu_renderer = r;
    });
    return () => {
      alive = false;
      gpu_renderer = null;
    };
  }, []);
  return <canvas ref={ref} className="absolute inset-0 h-full w-full" style={{ pointerEvents: "none", zIndex: 0 }} />;
}
