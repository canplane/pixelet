// @pxl/web/modules/world/render/lit_gltf_material.ts
//
// Shared by decor (decor/instanced_model.ts) and characters
// (characters/use_character_model.ts) — both are plain glTF-standard
// geometry (position/normal/uv, no baked per-vertex attributes the way
// chunk_stage.tsx's terrain/objects mesh has), textured with one flat
// swatch/skin texture, and need the exact same "lit, with an emissive
// floor so it stays visible with no guaranteed scene light, plus rim
// light" material recipe. chunk_stage.tsx's own material stays its own
// thing — it shares only the rim-light *term* itself (rim_light_shader.ts),
// not this whole recipe, since the rest of its shader work (baked
// a_rgb/a_layer_taps/a_visible attributes, texture-array layer blending)
// is genuine terrain-only data flow that doesn't generalize to "a mesh
// with one texture." This file is the level that actually was duplicated
// (decor and characters had, or would have had, the identical ~15 lines);
// cube's own bespoke pipeline was never that.
//
// Deliberately does NOT import app/settings/settings_store.ts, matching
// instanced_model.ts's own reason for avoiding it: that store composes in
// a slice reaching a `*.glb?url` asset import Vite handles but `deno test`
// has no loader for, and this module (like instanced_model.ts) is meant to
// stay importable from test files without dragging that chain in. Callers
// that need the live show_rim_light setting own that subscription
// themselves (decor/asset_store.ts for decor; a characters/ equivalent is
// this file's own reason for existing).

import * as THREE from "three";

import { light_cfg, rim_cfg } from "@/modules/world/environment/config.ts";
import { inject_rim_light_shader } from "./rim_light_shader.ts";

export function make_lit_gltf_material(texture: THREE.Texture, program_cache_key?: string): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({
    map: texture,
    roughness: 1,
    metalness: 0,
    // Same emissive-floor treatment every other lit surface gets
    // (chunk_stage.tsx's own inject_chunk_shader, hover_cursor.tsx's own
    // object ghost) — a minimum brightness that still shows the real
    // texture pattern rather than a flat tinted rectangle.
    emissive: 0xffffff,
    emissiveMap: texture,
    emissiveIntensity: light_cfg.FLOOR,
  });

  // Optional, and a key shared across materials is a trap rather than an
  // optimisation. A material that finds a program already cached under the
  // key never compiles: its onBeforeCompile never runs, and it draws with
  // whatever defines the first material to compile had. This recipe sets
  // `emissive: 0xffffff` and depends on the emissiveMap below to modulate
  // it, so inheriting a program without USE_EMISSIVEMAP renders that white
  // literally — which is what white decor models turned out to be.
  //
  // Give distinct materials distinct keys. Whatever sharing that gives up
  // is mostly illusory, since three.js keys programs by the standard
  // parameters as well, and materials that genuinely match will share one
  // without being told to.
  if (program_cache_key) material.customProgramCacheKey = () => program_cache_key;

  // This module doesn't know the live show_rim_light setting (see this file's
  // own top comment for why), so the value arrives from outside via
  // set_rim_intensity — and is replayed here, because a caller has no way to
  // know when this runs. onBeforeCompile fires at the material's first draw,
  // which is long after the caller has finished building it.
  material.onBeforeCompile = (shader) => {
    inject_rim_light_shader(shader);
    shader.uniforms.u_rim_intensity.value = rim_intensity_of(material);
    material.userData.shader = shader;
  };

  return material;
}

const RIM_INTENSITY_KEY = "rim_intensity";

/** Falls back to the configured intensity, not to zero — settings default to
 * on, so a caller that never learned the setting should look like the setting
 * is on rather than silently render without the term. */
function rim_intensity_of(material: THREE.MeshStandardMaterial): number {
  return (material.userData[RIM_INTENSITY_KEY] as number | undefined) ?? rim_cfg.INTENSITY;
}

/**
 * Pushes a show_rim_light value onto a material built by
 * make_lit_gltf_material, whenever the caller happens to know it.
 *
 * The value is *remembered on the material* and only then written to the
 * shader, because the shader may not exist yet: `userData.shader` is
 * populated by onBeforeCompile, which three fires at the material's first
 * draw. Every caller sets this while building the material, which is always
 * before that.
 *
 * This used to be a bare write that silently did nothing in that window, on
 * the recorded assumption that it was a gap of "the first render or two".
 * It was not a window, it was the only time anyone called: decor set it once
 * at load and never again, so decor shipped with no rim light at all —
 * `u_rim_intensity` measured 0 on all five of its materials against 0.1 on
 * terrain — while its shader inspected as correctly patched. Toggling the
 * setting was the only thing that ever turned it on.
 */
export function set_rim_intensity(material: THREE.MeshStandardMaterial, intensity: number): void {
  material.userData[RIM_INTENSITY_KEY] = intensity;
  const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
  if (shader) shader.uniforms.u_rim_intensity.value = intensity;
}
