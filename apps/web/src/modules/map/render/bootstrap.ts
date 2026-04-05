// @pxl/web/modules/map/render/bootstrap.ts

import * as THREE from "three/webgpu";

/**
 * Map renderer bootstrap.
 *
 * ROLE:
 * - owns renderer construction policy for the map scene
 * - keeps Canvas-level renderer init out of MapScene
 * - provides a single migration seam toward custom root / offscreen later
 *
 * NOTES:
 * - WebGPURenderer prefers WebGPU and falls back to WebGL2 when needed
 * - R3F v9 allows async gl callbacks, so renderer.init() can live here
 * - keep this file renderer-only; scene graph and camera logic stay elsewhere
 */
export async function create_map_renderer(props: any): Promise<THREE.WebGPURenderer> {
  const renderer = new THREE.WebGPURenderer({
    ...props,

    antialias: false,
    alpha: false,
    stencil: false,
    depth: true,
    powerPreference: "high-performance",
  });

  await renderer.init();

  /**
   * Match the usual Canvas defaults explicitly.
   * Once renderer bootstrap is externalized, color/tone policy should live here.
   */
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;

  return renderer;
}
