// pxl-render/light_mark.wgsl

// The world's receivers of the shadow map: the drawn pixels' rays, a thread each. The
// tile the point a ray met the world at lies on is marked stood on (`light::stands_at`),
// and the tiles between it and where each pixel after it met it, across, down and either
// diagonal, each pair of neighbors once, its footprint (`light::FOOTPRINT_TILES`), so a
// jittered ray stands on a tile marked whichever way it moves, where the head asks it
// (`light::HEAD_FOOTPRINT`), else its own tile alone; nothing where it met nothing. A word
// is written alike by every thread it is marked by, so none needs to add, and only by the
// first to find it unmarked (`stand`), each tile and each pair's span once a workgroup. Run
// after the eye's rays, before the propagation (light_propagate.wgsl); the drawn receivers
// stand apart (light_mark_draws.wgsl).

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

/// Whether neighbors that met the world on tiles `a` and `b` mark the tiles between them: where
/// they lie within a footprint of each other, not on one tile (`light::footprint`).
fn spans(a: vec2<i32>, b: vec2<i32>) -> bool {
  let d = abs(a - b);
  return !any(d > vec2<i32>(LIGHT_FOOTPRINT_TILES)) && any(d != vec2<i32>(0));
}

/// The tiles between `a` and `b` marked stood on (`spans`).
fn stand_between(current: u32, a: vec2<i32>, b: vec2<i32>) {
  let lo = min(a, b);
  let hi = max(a, b);
  // Only the box's tiles are marked (`stand`), so the walk keeps to them.
  let n = i32(light_tiles());
  let first = max(lo, vec2<i32>(0));
  let last = min(hi, vec2<i32>(n - 1i));
  for (var j = first.y; j <= last.y; j += 1i) {
    for (var i = first.x; i <= last.x; i += 1i) {
      stand(current, vec2<i32>(i, j));
    }
  }
}

/// The squares' met tiles, each pixel's worked out once: the workgroup's own and the column after
/// it and the rows before and after, as its pixels' neighbors after them reach (`MET_SIDE` across,
/// `MET_SIDE + 1` down from the row before): the tile, then 1 where its ray met the world.
const MET_SIDE: u32 = SQUARE_SIDE + 1u;
const MET_CELLS: u32 = MET_SIDE * (SQUARE_SIDE + 2u);
var<workgroup> met: array<vec3<i32>, MET_CELLS>;

/// The met tile of the square's pixel `l`, from the workgroup's first, its row `-1` the one before.
fn met_at(l: vec2<i32>) -> Met {
  let k = met[u32(l.y + 1i) * MET_SIDE + u32(l.x)];
  return Met(k.z != 0i, k.xy);
}

/// The square's pixel numbered `k`, row by row, as its lanes are numbered.
fn square_px(k: u32) -> vec2<i32> {
  return vec2<i32>(i32(k % SQUARE_SIDE), i32(k / SQUARE_SIDE));
}

/// Neighbor `j` after the square's pixel `l`: across, down and the two diagonals, each step
/// worked out, not read off a table of vectors, which Adreno's driver refuses
/// (doc/experiment/20261103-shaders-on-two-phones).
fn after(l: vec2<i32>, j: u32) -> vec2<i32> {
  return l + vec2<i32>(select(1i, 0i, j == 1u), select(select(1i, -1i, j == 3u), 0i, j == 0u));
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_mark_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
  @builtin(local_invocation_index) index: u32,
) {
  let drawn = textureDimensions(rays);
  let first = vec2<i32>(group.xy * SQUARE_SIDE);
  for (var k = index; k < MET_CELLS; k += SQUARE_SIDE * SQUARE_SIDE) {
    let p = first + vec2<i32>(i32(k % MET_SIDE), i32(k / MET_SIDE) - 1i);
    var cell = vec3<i32>(0i);
    if (p.y >= 0i) {
      let m = met_tile(vec2<u32>(p), drawn);
      cell = vec3<i32>(m.tile, select(0i, 1i, m.got));
    }
    met[k] = cell;
  }
  workgroupBarrier();
  let l = vec2<i32>(lane.xy);
  let own = met_at(l);
  if (!own.got) {
    return;
  }
  // Its tile, by the first of the square's pixels that met the world on it alone: a store a
  // tile, not a pixel, where a tile holds many.
  let current = cast_number();
  var first_on = true;
  for (var k = 0u; k < index && first_on; k += 1u) {
    let o = met_at(square_px(k));
    first_on = !(o.got && all(o.tile == own.tile));
  }
  if (first_on) {
    stand(current, own.tile);
  }
  if (!footprint()) {
    return;
  }
  // The tiles between it and each neighbor after it, so every pair of neighbors is taken once,
  // each span by the first pair of the square that spans it alone.
  for (var j = 0u; j < 4u; j += 1u) {
    let other = met_at(after(l, j));
    if (!other.got || !spans(own.tile, other.tile)) {
      continue;
    }
    let lo = min(own.tile, other.tile);
    let hi = max(own.tile, other.tile);
    var first_pair = true;
    for (var k = 0u; k < 4u * index + j && first_pair; k += 1u) {
      let a = met_at(square_px(k / 4u));
      let b = met_at(after(square_px(k / 4u), k % 4u));
      first_pair = !(a.got && b.got && spans(a.tile, b.tile)
        && all(min(a.tile, b.tile) == lo) && all(max(a.tile, b.tile) == hi));
    }
    if (first_pair) {
      stand_between(current, own.tile, other.tile);
    }
  }
}
