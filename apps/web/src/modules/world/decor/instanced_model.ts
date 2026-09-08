// @pxl/web/modules/world/decor/instanced_model.ts

/**
 * General "many copies of a small static GLB" renderer — the piece this
 * whole decor/ pipeline exists to establish as a reusable pattern (trees,
 * boats, cars, and trains today; a future fence/candle/etc. kind loads its
 * own variants through this exact same hook rather than cloning full scene
 * graphs the way characters/use_character_model.ts does, which is fine at
 * "one companion" scale but not at "hundreds of scattered instances"
 * scale). Each variant gets its own THREE.InstancedMesh (three.js requires
 * one geometry per mesh) and its own material — variants are allowed to
 * come from entirely different Kenney packs with different colormap
 * atlases (tree_decor.ts mixes fantasy-town-kit and graveyard-kit trees
 * this way), so nothing here assumes one shared texture across a kind's
 * whole variant list.
 *
 * Every material is lit (MeshStandardMaterial), matching chunk_stage.tsx's
 * own baked cube content — a plain MeshBasicMaterial used to be the choice
 * here (2026-09-05, "no guarantee of real scene lights bright enough for a
 * lit material"), but environment/config.ts's own rim_cfg doc comment
 * already states the actual governing principle: "a shared render pipeline
 * shouldn't treat one geometry role differently from another for the same
 * shader term" — decor staying unlit-and-rim-less was that principle not
 * having reached this file yet, not a deliberate exception. The same
 * emissive-floor treatment every other lit surface gets (light_cfg.FLOOR)
 * covers the original "no guaranteed light" concern — see
 * discussions/20260906-decor-lit-shading.md.
 *
 * Deliberately does NOT import app/settings/settings_store.ts here (unlike
 * chunk_stage.tsx's own create_material()) even though u_rim_intensity needs
 * to track its show_rim_light setting: that store composes in
 * settings/slices/character_slice.ts, which reaches characters/variants.ts's
 * own `*.glb?url` asset imports — fine for Vite, but `deno test` has no
 * loader for that specifier, and this file is imported directly by
 * decor_hit.test.ts/instanced_model.test.ts. Pulling that whole chain into a
 * module those tests import would break `deno task test` outright, not just
 * slow it down. decor_layer.tsx's own useAllDecorBundleAssets — the only
 * real (non-test) consumer that needs live updates — owns that
 * subscription instead, one level up from here.
 */

import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { mergeBufferGeometries } from "three-stdlib";

import type { f64 } from "@xpute/core/kit/word.ts";
import { load_gltf, load_texture } from "@/modules/world/gltf_cache.ts";
import { make_lit_gltf_material } from "@/modules/world/render/lit_gltf_material.ts";

/**
 * Merges every mesh found in a loaded glTF scene into one static
 * BufferGeometry, each baked with its own node's world transform first —
 * not every model this pipeline loads is Kenney's simplest single-mesh
 * shape (a tree): the car-kit's own vehicles are a body node plus four
 * separate wheel nodes, all sharing one material, built that way upstream
 * so a real engine could spin the wheels independently. This renderer never
 * animates a scattered background decoration, so collapsing them into one
 * static mesh up front costs nothing and keeps every variant, single-mesh
 * or not, fitting the same "one geometry, one InstancedMesh" shape the
 * rest of this file assumes.
 */
function merge_scene_geometry(root: THREE.Object3D): THREE.BufferGeometry | null {
  root.updateMatrixWorld(true);
  const parts: THREE.BufferGeometry[] = [];
  root.traverse((node) => {
    if (!(node instanceof THREE.Mesh)) return;
    const part = node.geometry.clone();
    part.applyMatrix4(node.matrixWorld);
    parts.push(part);
  });
  if (parts.length === 0) return null;
  const merged = parts.length === 1 ? parts[0] : mergeBufferGeometries(parts, false);
  if (parts.length > 1) { for (const part of parts) part.dispose(); }
  return merged;
}

export type DecorVariantAsset = {
  model_url: string;
  /** Not necessarily shared across a kind's other variants — see this
   * file's own doc comment. */
  skin_url: string;
};

export type LoadedModelVariants = {
  /** One entry per variants input, in the same order; null until that
   * variant's own load resolves. Geometry is never mutated after load, so
   * it's safe to feed straight into an InstancedMesh with no cloning —
   * unlike character models, there's no skeleton/skinning here to rebind
   * per instance. */
  geometries: (THREE.BufferGeometry | null)[];
  /** One material per variant, null until that variant's own skin texture
   * has loaded. */
  materials: (THREE.MeshStandardMaterial | null)[];
};

/**
 * Loads every variant's model+skin (each independently cached by URL — see
 * gltf_cache.ts) and exposes ready-to-use InstancedMesh inputs. A glTF's
 * own material is never used: GLTFLoader tries (and fails) to resolve the
 * packed colormap's own relative "Textures/colormap.png" reference once
 * Vite content-hashes the .glb's filename — same failure
 * use_character_model.ts's own doc comment documents for character skins,
 * fixed the same way here (override the material after load instead of
 * trusting the glTF's own).
 */
export function useInstancedModelVariants(variants: readonly DecorVariantAsset[]): LoadedModelVariants {
  const [geometries, set_geometries] = useState<(THREE.BufferGeometry | null)[]>(() => variants.map(() => null));
  const [materials, set_materials] = useState<(THREE.MeshStandardMaterial | null)[]>(() => variants.map(() => null));

  // variants is expected to be a stable module-level constant (every
  // current DecorKind's own list — see tree_decor.ts) — joined into one key
  // so this effect only re-fires if the actual variant set changes, not on
  // every render of a caller that inlines a fresh array literal.
  const variants_key = variants.map((v) => `${v.model_url}:${v.skin_url}`).join("|");

  useEffect(() => {
    let cancelled = false;
    set_geometries(variants.map(() => null));
    variants.forEach((variant, i) => {
      load_gltf(variant.model_url).then((gltf) => {
        if (cancelled) return;
        const geometry = merge_scene_geometry(gltf.scene);
        set_geometries((current) => {
          const next = current.slice();
          next[i] = geometry;
          return next;
        });
      });
    });
    return () => {
      cancelled = true;
    };
    // variants_key (not variants) is the real dependency — variants itself
    // is a fresh array identity each render even when its contents match.
    // deno-lint-ignore-next-line
  }, [variants_key]);

  useEffect(() => {
    let cancelled = false;
    set_materials(variants.map(() => null));
    variants.forEach((variant, i) => {
      load_texture(variant.skin_url).then((texture) => {
        if (cancelled) return;
        texture.colorSpace = THREE.SRGBColorSpace;
        texture.flipY = false;
        // Distinct per-variant skin textures mean distinct materials
        // regardless (map/emissiveMap differ), but the compiled WebGLProgram
        // itself is identical for every one of them — same reasoning
        // chunk_stage.tsx's own create_material() gives for "chunk-mesh",
        // hence the shared cache key (decor's own geometry never has
        // skinning attributes, unlike characters/use_character_model.ts's
        // own call into this same helper — see lit_gltf_material.ts's own
        // doc comment on why that one doesn't pass one).
        const material = make_lit_gltf_material(texture, "decor-mesh");
        // this file doesn't know the live show_rim_light setting (see this
        // file's own top doc comment for why) — the real value gets set
        // from outside, by whichever caller actually needs it live
        // (decor_layer.tsx's own useAllDecorBundleAssets, via
        // lit_gltf_material.ts's own set_rim_intensity).
        set_materials((current) => {
          const next = current.slice();
          next[i] = material;
          return next;
        });
      });
    });
    return () => {
      cancelled = true;
    };
    // deno-lint-ignore-next-line
  }, [variants_key]);

  return useMemo(() => ({ geometries, materials }), [geometries, materials]);
}

export type ModelInstanceTransform = {
  position: readonly [f64, f64, f64];
  /** Radians, applied about world Z after the model's own Y-up -> Z-up fix
   * (see compose_instance_matrix's own doc comment) — this world already
   * treats Z as up throughout (get_ele_at_local, every V3, ...), while
   * every glTF asset here is Y-up by spec. */
  rotation_z: f64;
  scale: f64;
  /** This instance's own objects/store.ts leaf id, or null for a procedural
   * (scatter.ts-derived, never-stored) placement — decor_hit.ts's own
   * raycast_decor uses this to resolve a real mesh hit's own faceIndex back
   * to a selectable/erasable entry, and to tell a procedural placement
   * apart from one (it has no id, so it stays transparent to hit-testing,
   * exactly as it always has been). */
  id: string | null;
};

const Y_UP_TO_Z_UP = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), Math.PI / 2);
const _position = new THREE.Vector3();
const _quaternion = new THREE.Quaternion();
const _z_spin = new THREE.Quaternion();
const _scale = new THREE.Vector3();

/** Bakes the Y-up -> Z-up axis fix directly into the instance matrix — a
 * merged/baked mesh's own instances are real, separately-transformed vertex
 * copies, not nested Object3Ds, so there's no separate wrapper-group to hold
 * this the way use_character_model.ts's up_axis_fix Group does for a single
 * companion. */
export function compose_instance_matrix(out: THREE.Matrix4, transform: ModelInstanceTransform): THREE.Matrix4 {
  _position.set(transform.position[0], transform.position[1], transform.position[2]);
  _z_spin.setFromAxisAngle(new THREE.Vector3(0, 0, 1), transform.rotation_z);
  _quaternion.copy(_z_spin).multiply(Y_UP_TO_Z_UP);
  _scale.setScalar(transform.scale);
  return out.compose(_position, _quaternion, _scale);
}

/**
 * Bakes every one of `instances`' own real, separately-transformed copies
 * of `geometry` into one merged BufferGeometry — the same "each block
 * contributes its own real geometry into one merged per-chunk buffer"
 * baking model chunk_mesher.ts's own cube bake uses, not a shared primitive
 * restyled per instance (what an InstancedMesh would do instead). Returns
 * null for zero instances — callers already gate mounting on a nonempty
 * instance list (chunk_stage.tsx's own Chunk, in its bundle_assets.map()
 * JSX), so this is a defensive fallback, not an expected path.
 *
 * `BufferGeometry.applyMatrix4` (not chunk_mesher.ts's own hand-rolled
 * position math) does the real transform work here: unlike that module's
 * own boxes, whose normals are always axis-aligned under a diagonal-only
 * scale (so direction survives untouched — see that module's own doc
 * comment), an arbitrary GLB's normals have no such guarantee, and
 * three.js's own applyMatrix4 already applies the correct inverse-transpose
 * normal-matrix correction for a general transform.
 */
export function bake_decor_instances(geometry: THREE.BufferGeometry, instances: readonly ModelInstanceTransform[]): THREE.BufferGeometry | null {
  if (instances.length === 0) return null;
  const scratch = new THREE.Matrix4();
  const clones: THREE.BufferGeometry[] = [];
  for (const instance of instances) {
    compose_instance_matrix(scratch, instance);
    const clone = geometry.clone();
    clone.applyMatrix4(scratch);
    clones.push(clone);
  }
  const merged = clones.length === 1 ? clones[0] : mergeBufferGeometries(clones, false);
  if (clones.length > 1) { for (const clone of clones) clone.dispose(); }
  merged?.computeBoundingSphere();
  return merged;
}
