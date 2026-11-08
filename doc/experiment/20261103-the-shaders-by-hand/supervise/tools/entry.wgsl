
@group(0) @binding(12) var<storage, read> asked: array<u32>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;
@compute @workgroup_size(64)
fn world_asked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let a = id.x * 20u;
  if (a >= arrayLength(&asked)) { return; }
  let at = vec4<u32>(asked[a + 7u], asked[a + 8u], asked[a + 9u], asked[a + 10u]);
  let c = vec3<i32>(i32(asked[a + 2u]), i32(asked[a + 3u]), i32(asked[a + 4u]));
  let carve = vec4<i32>(i32(asked[a + 11u]), i32(asked[a + 12u]), i32(asked[a + 13u]), i32(asked[a + 14u]));
  let hide = vec4<i32>(i32(asked[a + 15u]), i32(asked[a + 16u]), i32(asked[a + 17u]), i32(asked[a + 18u]));
  let r = world_node(world_child(world_root(asked[a + 1u], c), asked[a + 1u], c), asked[a + 1u], c, asked[a + 5u], asked[a + 6u], carve, hide);
  told[id.x] = r.kind + r.asset.x + r.at.x + hollow(asked[a + 1u], c);
}
