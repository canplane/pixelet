// pxl-render/light_slide.wgsl

// The tiles of the box an earlier cast of the epoch cast, where the box slid since, a
// `TILE`-aligned corner at a time (gpu/shadow.rs `aim`): a thread a tile, after the host
// moved the reads with the box, each tile's word the word of the tile that held its world
// texels under the corner before (`light::HEAD_CORNER_WAS`; gpu/ray/cast.rs `prepare`). A
// tile the box slid onto, and one the slide moved into another band (`Light::keeps`), is
// set to none, so the propagation takes it, read, as read anew (light_propagate.wgsl); the
// rest hold what their cast left. Each writes its own word alone. Run only where the box
// slid.

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_slide_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let tiles = light_tiles();
  if (id.x >= tiles || id.y >= tiles) {
    return;
  }
  let was = vec2<i32>(i32(head_word(LIGHT_HEAD_CORNER_WAS)), i32(head_word(LIGHT_HEAD_CORNER_WAS + 1u)));
  if (!keeps_tile(id.xy, was)) {
    list_put(reads_at() + id.y * tiles + id.x, 0u);
  }
}
