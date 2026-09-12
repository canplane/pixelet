// @pxl/web/module/world/render/rim_light_shader.ts

/**
 * Fresnel/rim-light term, shared by every lit material in the world —
 * factored out of chunk_stage.tsx's own inject_chunk_shader once decor's own
 * material (instanced_model.ts) needed the identical rim treatment without
 * that function's terrain-specific baked-attribute machinery
 * (a_rgb/a_layer_taps/a_visible/a_local_pos/a_box_scale — decor's geometry
 * carries none of those, just a plain glTF-standard position/normal/uv).
 * environment/config.ts's own rim_cfg doc comment already states the
 * principle this follows: "a shared render pipeline shouldn't treat one
 * geometry role differently from another for the same shader term" — decor
 * not getting this was the one geometry role that principle hadn't reached
 * yet (see issue/20260906-decor-lit-shading.md).
 *
 * Needs nothing beyond what MeshStandardMaterial's own meshphysical shader
 * chunks already provide (`mvPosition`, `vNormal`) plus the two uniforms this
 * installs — safe to call from any MeshStandardMaterial's onBeforeCompile,
 * baked chunk mesh, instanced decor or plain glTF mesh alike. Composes with a
 * caller's own further shader.vertexShader/fragmentShader edits regardless of
 * call order: every replace target here keeps its own marker text
 * (`#include <common>`, `#include <project_vertex>`) present in the
 * substituted string, the same chaining pattern chunk_material.ts's own
 * remaining terrain-specific replacements already rely on for themselves.
 */

import * as THREE from "three";

import { rim_cfg } from "@/module/world/environment/config.ts";

// GLSL ES 3.00 has no int-to-float overload for arithmetic on a float/vector
// operand (pow(float, int), vec3 * int, etc. simply fail to compile) — a
// whole-number config constant like rim_cfg.POWER=1 template-interpolates as
// the bare token "1", not "1.0", and silently takes down the entire shader
// (every mesh sharing the material goes blank, no visible error unless the
// console is checked). Route every numeric config constant injected into
// GLSL through this so a future whole-number tuning value can't reintroduce
// the same compile failure.
export function glsl_float(n: number): string {
  return Number.isInteger(n) ? `${n}.0` : `${n}`;
}

export function inject_rim_light_shader(shader: {
  vertexShader: string;
  fragmentShader: string;
  uniforms: Record<string, THREE.IUniform>;
}): void {
  shader.uniforms.u_rim_color = { value: new THREE.Color(rim_cfg.COLOR) };
  // Starts at 0, not rim_cfg.INTENSITY — the caller's own settings-driven
  // effect sets the real value from app/setting/store.ts's
  // show_rim_light once mounted (and on every toggle after), same
  // "compiled in unconditionally, toggled via a uniform" reasoning
  // chunk_stage.tsx's own u_low_spec uses: flipping the setting never needs
  // a shader recompile.
  shader.uniforms.u_rim_intensity = { value: 0 };
  shader.vertexShader = shader.vertexShader
    .replace("#include <common>", `#include <common>\nvarying vec3 v_rim_view_pos;`)
    // Read off `mvPosition` rather than recomputing `modelViewMatrix *
    // position`. They are the same thing for a plain mesh and not for an
    // instanced one: three applies `instanceMatrix` inside this chunk, so a
    // hand-rolled version places every instance at the mesh's own origin and
    // the rim reads as if the whole field were one object. Decor became
    // instanced (decor/field.ts) and this is where that would have gone
    // silently wrong. Skinning and morphing are folded into `transformed`
    // by the same route, for free.
    .replace("#include <project_vertex>", `#include <project_vertex>\n  v_rim_view_pos = -mvPosition.xyz;`);
  // vViewPosition (a standard-sounding name) is NOT available here: it only
  // exists in the Lambert/Phong/Toon shader chunks, never in meshphysical's
  // own lights_physical_pars_fragment.glsl.js, which MeshStandardMaterial/
  // MeshPhysicalMaterial both compile against — referencing it would be an
  // undeclared-identifier compile error (rim silently never applying,
  // regardless of u_rim_intensity), hence this own v_rim_view_pos varying
  // instead of assuming a standard one that doesn't exist for this material.
  // pow(1 - N.V, POWER) is the classic fresnel/rim falloff: near 0 face-on
  // (N.V close to 1), rising toward 1 right at the silhouette (N.V close to
  // 0).
  shader.fragmentShader = shader.fragmentShader
    .replace("#include <common>", `#include <common>\nvarying vec3 v_rim_view_pos;\nuniform vec3 u_rim_color;\nuniform float u_rim_intensity;`)
    .replace(
      "vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;",
      `vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;
      vec3 rim_view_dir = normalize(v_rim_view_pos);
      float rim = pow(1.0 - max(dot(normalize(vNormal), rim_view_dir), 0.0), ${glsl_float(rim_cfg.POWER)});
      outgoingLight += u_rim_color * rim * u_rim_intensity;`,
    );
}
