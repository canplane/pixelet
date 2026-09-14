// @pxl/web/render/gltf/gltf.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import { GltfPath, parse_gltf, scene_nodes, world_matrices } from "./gltf.ts";

const ASSET = new URL("../../../../../asset/", import.meta.url);
const read = async (path: string) => (await Deno.readFile(new URL(path, ASSET))).buffer as ArrayBuffer;

Deno.test("gltf - a rigged character comes out with its skins, weights and clips", async () => {
  const g = parse_gltf(await read("actor/kenney_mini_characters/male-f.glb"));
  const prims = g.meshes.flat();
  assertEquals(prims.length, 2);
  assertEquals(prims.reduce((a, p) => a + p.position.length / 3, 0), 1291);
  assertEquals(g.skins.map((s) => s.joints.length), [7, 7]);
  assertEquals(g.skins[0].inverse_bind.length, 7 * 16);
  for (const p of prims) {
    assert(p.joints && p.weights);
    for (let i = 0; i < p.weights.length; i += 4) {
      assertAlmostEquals(p.weights[i] + p.weights[i + 1] + p.weights[i + 2] + p.weights[i + 3], 1, 1e-3);
    }
    for (const j of p.joints) assert(j < 7);
  }
  const idle = g.animations.find((a) => a.name === "idle")!;
  assertAlmostEquals(idle.duration, 1.3333, 1e-3);
  assert(idle.channels.every((c) => g.skins[0].joints.includes(c.node)));
  assert(idle.channels.some((c) => c.path === GltfPath.ROTATION && c.values.length === c.times.length * 4));
});

Deno.test("gltf - an unrigged pet is animated by its nodes, each part a node with a mesh", async () => {
  const g = parse_gltf(await read("actor/kenney_cube_pets/cat.glb"));
  assertEquals(g.skins.length, 0);
  const walk = g.animations.find((a) => a.name === "walk")!;
  assertEquals(walk.channels.length, 24);
  const order = scene_nodes(g);
  // Parents before children, every node reached once.
  assertEquals(order.length, g.nodes.length);
  order.forEach((n, k) => {
    const parent = g.nodes[n].parent;
    if (parent >= 0) assert(order.indexOf(parent) < k);
  });
  assert(g.meshes.flat().every((p) => p.joints === null));
});

Deno.test("gltf - a node's world matrix composes its parents' transforms", async () => {
  const g = parse_gltf(await read("decor/kenney_car_kit/taxi.glb"));
  const worlds = world_matrices(g);
  for (const n of scene_nodes(g)) {
    const node = g.nodes[n];
    if (node.parent < 0) assertEquals(Array.from(worlds[n]), Array.from(node.local));
  }
  // The taxi stands on its wheels: the lowest vertex, through its node, is
  // below the body's origin and the highest well above it.
  let lo = Infinity, hi = -Infinity;
  for (const n of scene_nodes(g)) {
    const node = g.nodes[n];
    if (node.mesh < 0) continue;
    const w = worlds[n];
    for (const p of g.meshes[node.mesh]) {
      for (let i = 0; i < p.position.length; i += 3) {
        const y = w[1] * p.position[i] + w[5] * p.position[i + 1] + w[9] * p.position[i + 2] + w[13];
        lo = Math.min(lo, y);
        hi = Math.max(hi, y);
      }
    }
  }
  assert(lo < 0 && hi > 1);
});
