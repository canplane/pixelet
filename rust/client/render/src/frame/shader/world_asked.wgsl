// pxl-render/world_asked.wgsl

// Asks the world's one query word for word, with the asks and what the query tells laid in
// buffers of their own (tests/world.rs). No shader the page runs includes this part: build.rs
// assembles it from part.rs `WORLD_ASKED` for that test and for Tint
// (ts/tool/shader/shader.test.ts).

@group(0) @binding(12) var<storage, read> asked: array<u32>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn world_asked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let a = id.x * 19u;
  if (a >= arrayLength(&asked)) {
    return;
  }
  let op = asked[a];
  let zoom = asked[a + 1u];
  let c = vec3<i32>(i32(asked[a + 2u]), i32(asked[a + 3u]), i32(asked[a + 4u]));
  let at = vec3<u32>(asked[a + 7u], asked[a + 8u], asked[a + 9u]);
  var kind = 0u;
  var asset = vec2<u32>(0u);
  var inheritance = vec3<u32>(0u);
  switch op {
    case 0u: {
      let carve = vec4<i32>(i32(asked[a + 10u]), i32(asked[a + 11u]), i32(asked[a + 12u]), i32(asked[a + 13u]));
      let hide = vec4<i32>(i32(asked[a + 14u]), i32(asked[a + 15u]), i32(asked[a + 16u]), i32(asked[a + 17u]));
      let r = world_node(at, zoom, c, asked[a + 5u], asked[a + 6u], carve, hide);
      kind = r.kind;
      asset = r.asset;
      inheritance = r.inheritance;
    }
    case 1u: {
      inheritance = world_root(zoom, c);
    }
    case 2u: {
      inheritance = world_child(at, zoom, c);
    }
    default: {
      asset.x = hollow(zoom, c);
    }
  }
  let t = id.x * 8u;
  told[t] = kind;
  told[t + 1u] = asset.x;
  told[t + 2u] = asset.y;
  told[t + 3u] = inheritance.x;
  told[t + 4u] = inheritance.y;
  told[t + 5u] = inheritance.z;
}
