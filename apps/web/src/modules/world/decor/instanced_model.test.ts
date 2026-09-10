// @pxl/web/modules/world/decor/instanced_model.test.ts

import { assertAlmostEquals } from "@std/assert";
import * as THREE from "three";

import { compose_instance_matrix, type ModelInstanceTransform } from "./instanced_model.ts";

/* These used to test bake_decor_instances, which merged every instance into
 * one buffer per (chunk, variant). decor/field.ts writes the same transform
 * into an InstancedMesh's own matrix buffer instead, so the composed matrix is
 * now the whole of what a placement means — and the only place the Y-up ->
 * Z-up fix, the world-Z spin and the scale meet. */

function transform(over: Partial<ModelInstanceTransform> = {}): ModelInstanceTransform {
  return { position: [0, 0, 0], rotation_z: 0, scale: 1, id: null, ...over };
}

function corner(t: ModelInstanceTransform, local: THREE.Vector3): THREE.Vector3 {
  return local.clone().applyMatrix4(compose_instance_matrix(new THREE.Matrix4(), t));
}

Deno.test("compose_instance_matrix - a model's own Y-up becomes world Z", () => {
  // Every glTF asset here is authored Y-up while this world treats Z as up
  // throughout, and a merged/baked mesh has no wrapper group to hold that fix
  // the way use_character_model.ts's own up_axis_fix Group does.
  const up = corner(transform(), new THREE.Vector3(0, 1, 0));
  assertAlmostEquals(up.x, 0, 1e-6);
  assertAlmostEquals(up.y, 0, 1e-6);
  assertAlmostEquals(up.z, 1, 1e-6);
});

Deno.test("compose_instance_matrix - rotation_z spins about world Z, after the up-axis fix", () => {
  // A quarter turn takes the model's own +X onto world +Y. Applied about
  // world Z rather than the model's own axes, which is what makes a scattered
  // tree's heading independent of how its asset happened to be authored.
  const spun = corner(transform({ rotation_z: Math.PI / 2 }), new THREE.Vector3(1, 0, 0));
  assertAlmostEquals(spun.x, 0, 1e-6);
  assertAlmostEquals(spun.y, 1, 1e-6);
  assertAlmostEquals(spun.z, 0, 1e-6);
});

Deno.test("compose_instance_matrix - a model's own base stays on the ground under scale", () => {
  // scatter.ts varies scale per instance, and a decor model's origin is at
  // its base — so scaling must not lift or sink it.
  const base = corner(transform({ position: [3, 4, 5], scale: 2 }), new THREE.Vector3(0, 0, 0));
  assertAlmostEquals(base.x, 3, 1e-6);
  assertAlmostEquals(base.y, 4, 1e-6);
  assertAlmostEquals(base.z, 5, 1e-6);

  const top = corner(transform({ position: [3, 4, 5], scale: 2 }), new THREE.Vector3(0, 1, 0));
  assertAlmostEquals(top.z, 5 + 2, 1e-6);
});

Deno.test("compose_instance_matrix - position translates without touching the rest", () => {
  const spun = transform({ position: [10, -20, 30], rotation_z: 0.7, scale: 1.5 });
  const at_origin = transform({ rotation_z: 0.7, scale: 1.5 });
  const local = new THREE.Vector3(0.3, -0.6, 0.9);
  const moved = corner(spun, local);
  const still = corner(at_origin, local);
  assertAlmostEquals(moved.x - still.x, 10, 1e-6);
  assertAlmostEquals(moved.y - still.y, -20, 1e-6);
  assertAlmostEquals(moved.z - still.z, 30, 1e-6);
});
