// @pxl/web/modules/world/environment/sun_dir.ts

/**
 * Shared, mutable current-sun-direction vector.
 *
 * The terrain shader's diffuse lighting used to hardcode a fixed direction,
 * so the ground never tracked the sun position the rest of the scene (sky
 * dome, DirectionalLight) already animates per theme/time.
 *
 * Each of the (potentially dozens of) per-chunk ShaderMaterials has its own
 * uniforms object, so every chunk's u_light_dir points at this same Vector3
 * instance instead — environment.tsx mutates it once per frame, and every
 * chunk picks up the change for free with no per-chunk update loop.
 */

import * as THREE from "three";

export const SUN_DIR = new THREE.Vector3(0.4, 0.35, 1.0).normalize();
