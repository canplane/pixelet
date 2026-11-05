// pxl-render/light_mark.wgsl

// The world's receivers of the shadow map: the drawn pixels' rays, a thread each. The
// tile the point a ray met the world at lies on is marked stood on (`light::stands_at`),
// and the tiles between it and where each pixel after it met it, across, down and either
// diagonal, each pair of neighbors once, its footprint (`light::FOOTPRINT_TILES`), so a
// jittered ray stands on a tile marked whichever way it moves, where the head asks it
// (`light::HEAD_FOOTPRINT`), else its own tile alone; nothing where it met nothing. A word
// is written alike by every thread it is marked by, so none needs to add, and only by the
// first to find it unmarked (`stand`). Run after the eye's rays, before the propagation
// (light_propagate.wgsl); the drawn receivers stand apart (light_mark_draws.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

/// Where pixel `p`'s ray met the world: whether it did, and the tile it met it on. A
/// ray that met nothing leaves its first word zero (pxl-engine `ray::first_word`).
struct Met {
  got: bool,
  tile: vec2<i32>,
}

fn met_tile(p: vec2<u32>, drawn: vec2<u32>) -> Met {
  if (p.x >= drawn.x || p.y >= drawn.y) {
    return Met(false, vec2<i32>(0));
  }
  let first = textureLoad(rays, p, 0u).x;
  if (first == 0u || first == RAY_MISS) {
    return Met(false, vec2<i32>(0));
  }
  let t = bitcast<f32>(textureLoad(rays, p, 1u).x);
  let tan_at = px_tan(p, drawn);
  return Met(true, tile_of(eye_aim(tan_at).dir * t));
}

/// The tiles between `a` and `b` marked stood on, where they lie within a footprint of
/// each other; none past it, nor where they are one tile.
fn stand_between(current: u32, a: vec2<i32>, b: vec2<i32>) {
  let lo = min(a, b);
  let hi = max(a, b);
  if (any(hi - lo > vec2<i32>(LIGHT_FOOTPRINT_TILES)) || all(lo == hi)) {
    return;
  }
  for (var j = lo.y; j <= hi.y; j += 1i) {
    for (var i = lo.x; i <= hi.x; i += 1i) {
      stand(current, vec2<i32>(i, j));
    }
  }
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_mark_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let drawn = textureDimensions(rays);
  let own = met_tile(id.xy, drawn);
  if (!own.got) {
    return;
  }
  let current = cast_number();
  stand(current, own.tile);
  if (!footprint()) {
    return;
  }
  // The neighbors after it, so every pair of neighbors is taken once: across, down,
  // and the two diagonals, each step worked out, not read off a table of vectors,
  // which Adreno's driver refuses (doc/experiment/20261103-shaders-on-two-phones).
  for (var j = 0u; j < 4u; j += 1u) {
    let step = vec2<i32>(select(1i, 0i, j == 1u), select(select(1i, -1i, j == 3u), 0i, j == 0u));
    let q = vec2<i32>(id.xy) + step;
    if (q.y >= 0i) {
      let other = met_tile(vec2<u32>(q), drawn);
      if (other.got) {
        stand_between(current, own.tile, other.tile);
      }
    }
  }
}
