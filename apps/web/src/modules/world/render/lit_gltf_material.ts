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

import { light_cfg } from "@/modules/world/environment/config.ts";
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

  // u_rim_intensity starts at 0 (inject_rim_light_shader's own default) —
  // this module doesn't know the live show_rim_light setting (see this
  // file's own top comment for why); the real value gets set from outside
  // via set_rim_intensity, using the shader reference stashed in
  // userData.shader below.
  material.onBeforeCompile = (shader) => {
    inject_rim_light_shader(shader);
    material.userData.shader = shader;
  };

  return material;
}

/** Pushes a live show_rim_light value onto a material built by
 * make_lit_gltf_material — a no-op before that material's shader has
 * actually compiled once (userData.shader isn't populated until
 * onBeforeCompile fires), same "the first render or two just doesn't have
 * rim light yet" gap decor_layer.ts's own effect already accepts. */
export function set_rim_intensity(material: THREE.MeshStandardMaterial, intensity: number): void {
  const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
  if (shader) shader.uniforms.u_rim_intensity.value = intensity;
}
