// @pxl/web/modules/world/decor/decor_hit.ts

/**
 * Real mesh-based hit-testing for baked decor — replaces the earlier
 * exact-cell-lookup workaround camera/ray.ts used to carry (a "decor never
 * paints a voxel, so resolve whatever cell the ray struck instead" fallback,
 * necessary back when decor was InstancedMesh with no addressable
 * per-instance identity anyway). Now that decor bakes into a real THREE.Mesh
 * (instanced_model.ts's bake_decor_instances), a real THREE.Raycaster pass
 * against it is both more accurate (hits the model's own real silhouette,
 * not just "this cell") and the more honest fix once the geometry exists to
 * support it.
 *
 * A module-level registry, not a React context: camera/input.tsx's own
 * update_hover() needs to query "every currently mounted decor mesh"
 * synchronously from a plain (non-React) raycast helper outside the render
 * tree — same reasoning objects/store.ts's own module-level Maps use
 * instead of a context.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import * as THREE from "three";

import type { ModelInstanceTransform } from "./instanced_model.ts";

type DecorMeshEntry = { mesh: THREE.Mesh; instances: readonly ModelInstanceTransform[] };
const registry = new Set<DecorMeshEntry>();

/**
 * Registers one currently-mounted, currently-baked decor mesh
 * (decor_layer.tsx's own DecorVariantMesh) — call again (a fresh entry) on
 * every rebuild, since a rebuild changes both the mesh's own geometry
 * (bake_decor_instances) and the faceIndex -> instance mapping this module
 * depends on; the old entry must not linger once its own mesh/instances go
 * stale. Returns the unregister function.
 */
export function register_decor_mesh(mesh: THREE.Mesh, instances: readonly ModelInstanceTransform[]): () => void {
  const entry: DecorMeshEntry = { mesh, instances };
  registry.add(entry);
  return () => void registry.delete(entry);
}

/**
 * Which user-placed decor entry's own id (objects/store.ts leaf id) the
 * raycaster's current ray actually struck within max_dist, or null if
 * nothing did — INCLUDING when the closest real geometry hit belongs to a
 * procedural (scatter.ts-derived, never-stored, id-less) placement:
 * procedural decor has always been transparent to hit-testing (there was
 * never anything in objects/store.ts to select for it), so a hit there is
 * skipped in favor of whatever's next along the ray, not treated as "found,
 * but unselectable".
 *
 * faceIndex -> instance resolution relies on every instance sharing one
 * merged mesh's own single source geometry (bake_decor_instances is always
 * called once per (chunk, variant) — see decor_layer.tsx's own
 * DecorVariantMesh), so that mesh's own total face count divides evenly by
 * its own instance count into a constant faces-per-instance stride.
 *
 * `max_dist` bounds this to distances the caller's own already-found
 * terrain/object hit doesn't already beat — decor sitting on solid ground
 * can only ever be the right answer when it's the CLOSER thing along the
 * ray (in front of, not behind, whatever the abstract voxel raycast already
 * found there).
 */
export function raycast_decor(raycaster: THREE.Raycaster, max_dist: f64): string | null {
  if (registry.size === 0) return null;
  const entries = Array.from(registry);
  const hits = raycaster.intersectObjects(entries.map((entry) => entry.mesh), false);

  for (const hit of hits) {
    if (hit.distance > max_dist) break; // intersectObjects returns hits sorted nearest-first
    if (hit.faceIndex === undefined || hit.faceIndex === null) continue;
    const entry = entries.find((candidate) => candidate.mesh === hit.object);
    if (!entry || entry.instances.length === 0) continue;

    const geometry = entry.mesh.geometry;
    const total_faces: u32 = (geometry.index ? geometry.index.count : geometry.attributes.position.count) / 3;
    const faces_per_instance = total_faces / entry.instances.length;
    const instance_index = Math.floor(hit.faceIndex / faces_per_instance);
    const id = entry.instances[instance_index]?.id;
    if (id) return id;
  }
  return null;
}
