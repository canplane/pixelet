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
 * slow it down. asset_store.ts, which no test imports, owns the loading
 * and that subscription instead.
 */

import * as THREE from "three";
import { mergeBufferGeometries } from "three-stdlib";

import type { f64 } from "@xpute/core/kit/word.ts";

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
export function merge_scene_geometry(root: THREE.Object3D): THREE.BufferGeometry | null {
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

export type ModelInstanceTransform = {
  position: readonly [f64, f64, f64];
  /** Radians, applied about world Z after the model's own Y-up -> Z-up fix
   * (see compose_instance_matrix's own doc comment) — this world already
   * treats Z as up throughout (get_ele_at_local, every V3, ...), while
   * every glTF asset here is Y-up by spec. */
  rotation_z: f64;
  scale: f64;
  /** Present in the built set but not drawn: set_virtual_hide is
   * suppressing it while the eraser hovers it. It stays in the list because
   * hit testing runs against real geometry, so dropping it here would take
   * the entry out of hit testing too — and a hover that cannot find what it
   * is hovering releases the hide, which brings the entry back, which lets
   * the hover find it again. field.ts puts these in a second, never-rendered
   * instanced mesh for exactly that reason. */
  hidden?: boolean;
  /** This instance's own objects/store.ts leaf id, or null for a procedural
   * (scatter.ts-derived, never-stored) placement — decor_hit.ts's own
   * raycast_decor uses this to resolve a hit's own instanceId back to a
   * selectable/erasable entry, and to tell a procedural placement apart from
   * one (it has no id, so it stays transparent to hit-testing, exactly as it
   * always has been). */
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

/* bake_decor_instances lived here — every instance's own transformed copy of
 * the source geometry merged into one buffer per (chunk, variant), the same
 * baking model chunk_mesher.ts uses for cube content. It is gone with the
 * meshes it built: decor/field.ts draws one InstancedMesh per variant for the
 * whole world, so an instance is a matrix in a buffer rather than a vertex
 * copy, and a placement change rewrites one matrix instead of remerging every
 * copy in the chunk. Baking was the right shape while a chunk owned its own
 * decor mesh and the alternative was a mesh per variant per chunk; it stopped
 * being right when the mesh stopped belonging to the chunk. */
