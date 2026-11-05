// pxl-render/world_asked.wgsl

// The world's one query asked word for word, its asks and what is told in words of their
// own (tests/world.rs). In no shader the page runs: what build.rs `world_asked` assembles
// for that test and for Tint (ts/tool/shader/shader.test.ts).

@group(0) @binding(12) var<storage, read> asked: array<u32>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn world_asked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let a = id.x * 20u;
  if (a >= arrayLength(&asked)) {
    return;
  }
  let op = asked[a];
  let zoom = asked[a + 1u];
  let c = vec3<i32>(i32(asked[a + 2u]), i32(asked[a + 3u]), i32(asked[a + 4u]));
  let at = vec4<u32>(asked[a + 7u], asked[a + 8u], asked[a + 9u], asked[a + 10u]);
  var r = Answer(0u, vec2<u32>(0u), vec4<u32>(0u));
  switch op {
    case 0u: {
      let carve = vec4<i32>(i32(asked[a + 11u]), i32(asked[a + 12u]), i32(asked[a + 13u]), i32(asked[a + 14u]));
      let hide = vec4<i32>(i32(asked[a + 15u]), i32(asked[a + 16u]), i32(asked[a + 17u]), i32(asked[a + 18u]));
      r = world_node(at, zoom, c, asked[a + 5u], asked[a + 6u], carve, hide);
    }
    case 1u: {
      r.handed = world_root(zoom, c);
    }
    case 2u: {
      r.handed = world_child(at, zoom, c);
    }
    default: {
      r.asset.x = hollow(zoom, c);
    }
  }
  let t = id.x * 8u;
  told[t] = r.kind;
  told[t + 1u] = r.asset.x;
  told[t + 2u] = r.asset.y;
  told[t + 3u] = r.handed.x;
  told[t + 4u] = r.handed.y;
  told[t + 5u] = r.handed.z;
  told[t + 6u] = r.handed.w;
}
