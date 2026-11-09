// pxl-render/world_split.wgsl

// The world's one query for a node coarser than asked, and what a node hands the child a walk
// steps down to (pxl-engine world.rs `node`, `Inheritance::child`), built over world.wgsl's
// answer for a cell. A walk asks these; a pass that asks only cells does not include this part.

// A split's box, as pxl-engine answer.rs packs it.

/// The port of pxl-engine `answer::box_word`.
fn box_word(lo: vec3<u32>, hi: vec3<u32>) -> vec2<u32> {
  let l = min(lo, vec3<u32>(BOX_STEPS - 1u));
  let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 1u, hi <= l) - 1u;
  return vec2<u32>(
    l.x | (h.x << BOX_BITS) | (l.y << (2u * BOX_BITS)),
    h.y | (l.z << BOX_BITS) | (h.z << (2u * BOX_BITS)),
  );
}

fn box_none(b: vec2<u32>) -> bool {
  return all(b == BOX_NONE);
}

/// The port of pxl-engine `answer::box_join`.
fn box_join(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  if (box_none(a)) {
    return b;
  }
  if (box_none(b)) {
    return a;
  }
  return box_word(min(box_first(a), box_first(b)), max(box_past(a), box_past(b)));
}

// What is placed, as pxl-engine object.rs reads it.

/// The port of pxl-engine `object::object_box`, for what is placed under a split.
fn object_box(s: u32, zoom: u32) -> vec2<u32> {
  if (s == SLOT_NOTHING) {
    return BOX_NONE;
  }
  if (!is_folded(s)) {
    return BOX_WHOLE;
  }
  let base = first_child(s >> SLOT_SHIFT) & ~BRANCH;
  var low = 0u;
  var high = 0u;
  for (var k = 0u; k < 8u; k += 1u) {
    if (slot_at(base + k, zoom + 1u) != SLOT_NOTHING) {
      low |= ~k & 7u;
      high |= k;
    }
  }
  if ((low | high) == 0u) {
    return BOX_NONE;
  }
  let half = BOX_STEPS / 2u;
  let lows = vec3<u32>(low & 1u, (low >> 1u) & 1u, low >> 2u);
  let highs = vec3<u32>(high & 1u, (high >> 1u) & 1u, high >> 2u);
  return box_word(select(vec3<u32>(half), vec3<u32>(0u), lows != vec3<u32>(0u)), select(vec3<u32>(half), vec3<u32>(BOX_STEPS), highs != vec3<u32>(0u)));
}

/// The port of pxl-engine `object::cells_by`.
fn cells_by(s: u32, d: u32) -> bool {
  if (s == SLOT_NOTHING) {
    return false;
  }
  let word = tree_data(s >> SLOT_SHIFT);
  if (!is_folded(s)) {
    return voxel(word & 0xffffu);
  }
  let shift = FILL_BITS * (d + 1u);
  return (word >> 16u) + 2u > select(0u, FULL >> (shift & 31u), shift <= FULL_BITS);
}

/// The port of pxl-engine `object::whole`.
fn whole(s: u32, carve: vec4<i32>, zoom: u32, at: vec3<i32>) -> bool {
  if (is_folded(s)) {
    return false;
  }
  let cz = u32(carve.w);
  if (carve.w == NO_CUT.w || cz <= zoom) {
    return true;
  }
  return any(path(zoom, carve.xyz >> vec3<u32>(cz - zoom)) != path(zoom, at));
}

// The ground and the ways, as pxl-engine ground.rs and way.rs answer them.

fn coarsest(zoom: u32, far: u32) -> u32 {
  return max(far, zoom + 1u);
}

/// The highest face to which the zooms from `far` to `near` round the relief's top
/// `relief_hi` (pxl-engine `ground::slab`).
fn slab_over(zoom: u32, relief_hi: i32, far: u32, near: u32) -> i32 {
  let hi = select(relief_hi, max(relief_hi, SEA_LEVEL), zoom < SHARD_ZOOM);
  var over = EMPTY_HI;
  for (var z = coarsest(zoom, far); z <= near; z += 1u) {
    over = max(over, face_ele(z, top_of(z, hi).cells));
  }
  return over;
}

/// The port of pxl-engine `ground::meets`.
fn meets(span: vec2<i32>, zoom: u32, z: i32, near: u32) -> bool {
  let e = max(near, LEAF_CELL_ZOOM) - LEAF_CELL_ZOOM;
  let s = LEAF_CELL_ZOOM + e - zoom;
  let first = ((z + z_start(zoom)) << s) - (SEA_CELLS << e);
  return first < (span.y << e) && first + (1i << s) > (span.x << e);
}

/// The box step that the leaf cell `v` lies in, within the node at height `z` that is `shift`
/// zooms coarser than the leaves (pxl-engine `ground::box_along_z`).
fn z_step(v: i32, z: i32, shift: u32) -> i32 {
  let first = v + SEA_CELLS;
  let scaled = select(first << ((BOX_BITS - shift) & 31u), first >> ((shift - BOX_BITS) & 31u), shift >= BOX_BITS);
  return scaled - (z << BOX_BITS);
}

/// The port of pxl-engine `ground::box_along_z`.
fn box_along_z(span: vec2<i32>, zoom: u32, z_at: i32) -> vec2<u32> {
  if (span.x >= span.y) {
    return BOX_NONE;
  }
  if (zoom >= LEAF_CELL_ZOOM) {
    return BOX_WHOLE;
  }
  let shift = LEAF_CELL_ZOOM - zoom;
  let z = z_at + z_start(zoom);
  let first = z_step(span.x, z, shift);
  let past = select(z_step(span.y, z, shift), z_step(span.y - 1i, z, shift) + 1i, shift >= BOX_BITS);
  if (past <= 0i || first >= i32(BOX_STEPS)) {
    return BOX_NONE;
  }
  let lo = u32(clamp(first, 0i, i32(BOX_STEPS) - 1i));
  let hi = u32(clamp(past, 1i, i32(BOX_STEPS)));
  return box_word(vec3<u32>(0u, 0u, lo), vec3<u32>(BOX_STEPS, BOX_STEPS, hi));
}

/// The port of pxl-engine `way::slab`.
fn way_slab(bound: vec2<i32>, first: u32, near: u32) -> vec2<i32> {
  if (bound.x >= bound.y) {
    return vec2<i32>(EMPTY_LO, EMPTY_HI);
  }
  var span = vec2<i32>(EMPTY_LO, EMPTY_HI);
  for (var z = first; z <= near; z += 1u) {
    var got = bound;
    if (z < LEAF_CELL_ZOOM) {
      let k = LEAF_CELL_ZOOM - z;
      let half = 1i << (k - 1u);
      got = vec2<i32>(((bound.x + half) >> k) << k, -(((half - bound.y) >> k) << k));
    }
    span = vec2<i32>(min(span.x, got.x), max(span.y, got.y));
  }
  return span;
}

/// What pxl-engine `ground::node` answers for a node coarser than asked: its kind and asset,
/// the ways' bound read with the ground's own, and where that bound was held (`place`).
struct GroundSplit {
  kind: u32,
  asset: vec2<u32>,
  ways: vec2<i32>,
  place: u32,
}

/// The port of pxl-engine `ground::node` where the node is not asked as a cell (`is_cell`).
fn ground_split(shard: u32, place: u32, zoom: u32, c: vec3<i32>, far: u32, near: u32) -> GroundSplit {
  let none = vec2<i32>(EMPTY_LO, EMPTY_HI);
  if (is_past(shard)) {
    if (c.z >= SEA_LEVEL && zoom > SHARD_ZOOM) {
      return GroundSplit(KIND_CELL, vec2<u32>(VAL_VOID, 0u), none, LOOKUP_NONE);
    }
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, LOOKUP_NONE);
  }
  if (zoom < MIN_ZOOM) {
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, place);
  }
  let r = bound_over(place, min(coarsest(zoom, far), near), near, zoom, c.xy);
  if (!r.here) {
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, r.place);
  }
  let span = vec2<i32>(UNBOUNDED_LO, slab_over(zoom, r.b.y, far, near));
  if (meets(span, zoom, c.z, near)) {
    return GroundSplit(KIND_SPLIT, box_along_z(span, zoom, c.z), r.b.zw, r.place);
  }
  return GroundSplit(KIND_CELL, vec2<u32>(VAL_VOID, 0u), r.b.zw, r.place);
}

// The world's query, as pxl-engine world.rs states it.

/// What a node hands its child `c` at `zoom`, given what the node was handed, `above`
/// (pxl-engine `Inheritance::child`).
fn world_child(above: vec3<u32>, zoom: u32, c: vec3<i32>) -> vec3<u32> {
  return vec3<u32>(tree_child(above.x, zoom, path(zoom, c)), above.y, above.z);
}

/// The port of pxl-engine `world::node` for a node coarser than asked. What is placed splits
/// the node where it may fill it, and the ways split what the ground leaves air where they may.
/// A split's box is every source's own box joined.
fn world_split(above: vec3<u32>, shard: u32, zoom: u32, c: vec3<i32>, far: u32, near: u32) -> Answer {
  let put = above.x;
  let splits = cells_by(put, near - zoom);
  var placed = BOX_NONE;
  if (splits) {
    placed = object_box(put, zoom);
  }
  let g = ground_split(shard, above.z, zoom, c, far, near);
  let at = vec3<u32>(put, shard, g.place);
  let ways = way_slab(g.ways, coarsest(zoom, far), near);
  if (!splits && g.kind != KIND_SPLIT && !(g.kind == KIND_CELL && meets(ways, zoom, c.z, near))) {
    return Answer(g.kind, g.asset, at);
  }
  var own = BOX_NONE;
  if (g.kind == KIND_SPLIT) {
    own = g.asset;
  } else if (g.kind == KIND_LACKS) {
    own = BOX_WHOLE;
  }
  let joined = box_join(box_join(own, box_along_z(ways, zoom, c.z)), placed);
  if (box_none(joined)) {
    return Answer(KIND_CELL, vec2<u32>(VAL_VOID, 0u), at);
  }
  return Answer(KIND_SPLIT, joined, at);
}

/// What is in the node `c` at `zoom`, asked at the zooms from `far` to `near`, where `above` is
/// what the node over it handed it (pxl-engine `world::node`).
fn world_node(above: vec3<u32>, zoom: u32, c: vec3<i32>, far: u32, near: u32, carve: vec4<i32>, hide: vec4<i32>) -> Answer {
  let shard = shard_at(above.y, zoom, c.xy);
  if (zoom >= near || (zoom >= LEAF_CELL_ZOOM && whole(above.x, carve, zoom, c))) {
    return world_cell(above, shard, zoom, c, carve, hide);
  }
  return world_split(above, shard, zoom, c, far, near);
}
