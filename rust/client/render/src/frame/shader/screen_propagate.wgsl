// pxl-render/frame/shader/screen_propagate.wgsl

// Propagates the light's changed tiles to the screen (pass/propagate.rs). It is the light's
// propagation turned round: light_propagate.wgsl `light_propagate_cs` takes the screen's
// receivers to the light's tiles, and this pass takes the map tiles that changed this frame
// back to the pixels that read them. Where any texel a pixel's shadow taps read lies in a map
// tile that changed (gpu/tile.rs), the pixel's screen tile takes this frame's number, as a
// mark would set it (screen_mark.wgsl). The host reads nothing back: it already counts the
// frame this pass runs in as marked, from the marks it drew.
//
// A workgroup takes a screen tile, and a thread a square of its pixels. A square's world is the
// slab between the least and the most depth the world's pass last left there, as the depth
// bound keeps them (screen_bound.wgsl). The eye stands still in every frame this pass runs, so
// that depth is still right. The slab is taken into the light's map. The whole tile's slab is
// tested first, and a tile whose slab lies off every map tile the frame's marks cover stops
// there. Where a square's slab spans more than a few map tiles, each pixel is taken alone.
//
// Every test errs toward marking: a tile marked too many only costs work, and a tile missed
// is a failure.

// `TileGrid`, `tiles_age` and `tiles_in_damage` come from tile_word.wgsl, which spells word.rs
// `screen`'s rule.

// The uniform's fields are listed in word.rs `propagate`, and build.rs appends an accessor for
// each of them.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> propagate_words: array<vec4f, PROPAGATE_VEC4S>;
@group(0) @binding(2) var<storage, read> light_changed_at: array<u32>;
@group(0) @binding(3) var<uniform> light_tiles: TileGrid;
@group(0) @binding(4) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(5) var<uniform> tiles: TileGrid;
// The depth bound: the least and the most depth drawn in each square of `BOUND_STEP` pixels a
// side, as the last world pass left them (screen_bound.wgsl).
@group(0) @binding(6) var<storage, read> bounds: array<vec2f>;
// The map tiles the frame's marks cover, as the first tile across and down and the tile just
// past the last (gpu/tile.rs `TileGrid::new_damage_rect`). The host hands it, and the drawn marks
// widen it (tile_mark.wgsl). No map tile outside it changed this frame.
@group(0) @binding(7) var<storage, read> new_damage_rect: vec4u;

// Whether tile `t` of the map changed this frame, as its word says. A map tile is in damage
// only in the frame it changed (gpu/tile.rs `damage_frame_cnt`).
fn propagate_changed(t: vec2i) -> bool {
  let n = vec2i(light_tiles.cnt.xy);
  if (any(t < vec2i(0)) || any(t >= n)) {
    return false;
  }
  let ii = u32(t.y * n.x + t.x);
  return tiles_in_damage(light_tiles, light_changed_at[ii]);
}

// The depth the world's pass left at pixel `p` of a target of `size` pixels, or 0, the sky's,
// past its edge.
fn propagate_depth(p: vec2u, size: vec2u) -> f32 {
  if (any(p >= size)) {
    return 0.0;
  }
  return textureLoad(depth, vec2i(p), 0);
}

// Where the point `c`, in pixels of a target of `size` pixels, at the reversed depth `d`, lies
// on the light's map, in texels.
fn propagate_texel(c: vec2f, d: f32, size: vec2u) -> vec2f {
  let ndc = vec2f(c.x / f32(size.x) * 2.0 - 1.0, 1.0 - c.y / f32(size.y) * 2.0);
  let e = propagate_unproject() * vec4f(ndc, d, 1.0);
  let l = propagate_light_view_proj() * vec4f(e.xyz / e.w, 1.0);
  return vec2f(l.x * 0.5 + 0.5, 0.5 - l.y * 0.5) * propagate_size();
}

// The map tiles a receiver's taps read around the texels from `lo` to `hi`, as the first and
// the last tile across and down, held to one tile past the map each way.
fn propagate_range(lo: vec2f, hi: vec2f) -> vec4i {
  let n = vec2i(light_tiles.cnt.xy);
  let side = light_tiles.size.w;
  // The clamp is on integers, so the range stays within a tile past the map whatever floats it
  // came from.
  let a = clamp(vec2i(floor((lo - propagate_spread()) / side)), vec2i(-1), n);
  let b = clamp(vec2i(floor((hi + propagate_spread()) / side)), vec2i(-1), n);
  return vec4i(a, b);
}

// Whether any tile of `propagate_range` lies among the map tiles the frame's marks cover.
fn propagate_in_damage(lo: vec2f, hi: vec2f) -> bool {
  let r = propagate_range(lo, hi);
  let m = vec4i(new_damage_rect);
  return r.x < m.z && r.z >= m.x && r.y < m.w && r.w >= m.y;
}

// The box, in the map's texels, around the slab of pixels from `at` to `at + side` each way
// between the reversed depths `far` and `near`, found from the slab's eight corners taken into
// the map, as its least corner in `xy` and its most in `zw`. Whatever those pixels read lies
// within this box.
fn propagate_slab(at: vec2u, side: u32, far: f32, near: f32, size: vec2u) -> vec4f {
  var lo = vec2f(1e30);
  var hi = vec2f(-1e30);
  for (var k = 0u; k < 8u; k++) {
    let c = at + vec2u(k & 1u, (k >> 1u) & 1u) * side;
    let t = propagate_texel(vec2f(c), select(far, near, (k & 4u) != 0u), size);
    lo = min(lo, t);
    hi = max(hi, t);
  }
  return vec4f(lo, hi);
}

// The tile's least and most depth drawn, gathered from its squares' bounds as bits: a positive
// f32's bits keep its order, so the integer atomics order the depths right. Then whether the
// tile's slab lies on a map tile the marks cover.
var<workgroup> slab_far: atomic<u32>;
var<workgroup> slab_near: atomic<u32>;
var<workgroup> slab_in_damage: bool;

fn propagate_tiles(lo: vec2f, hi: vec2f) -> i32 {
  let r = propagate_range(lo, hi);
  return (r.z - r.x + 1) * (r.w - r.y + 1);
}

// Whether any map tile of `propagate_range` changed this frame.
fn propagate_rect(lo: vec2f, hi: vec2f) -> bool {
  let r = propagate_range(lo, hi);
  var hit = false;
  for (var y = r.y; y <= r.w; y++) {
    for (var x = r.x; x <= r.z; x++) {
      hit = hit || propagate_changed(vec2i(x, y));
    }
  }
  return hit;
}

// The most map tiles a square's box may span before each of its pixels is tested alone.
const PROPAGATE_RECT_CAP: i32 = 16;

@compute @workgroup_size(BOUND_SQUARES, BOUND_SQUARES, 1)
fn screen_propagate_cs(
  @builtin(workgroup_id) wg: vec3u,
  @builtin(local_invocation_id) l: vec3u,
  @builtin(local_invocation_index) li: u32,
) {
  let tile = wg.xy;
  if (any(tile >= tiles.cnt.xy)) {
    return;
  }
  let ti = tile.y * tiles.cnt.x + tile.x;
  let size = vec2u(textureDimensions(depth));
  let side = u32(tiles.size.w);
  let at = tile * side + l.xy * BOUND_STEP;
  if (li == 0u) {
    atomicStore(&slab_far, bitcast<u32>(2.0));
    atomicStore(&slab_near, 0u);
  }
  workgroupBarrier();
  // The square's least and most depth drawn. The bound leaves out the sky, which no shadow
  // falls on, so a square of sky alone holds 2 and 0 and adds nothing to the tile's.
  var b = vec2f(2.0, 0.0);
  if (all(at < size)) {
    b = bounds[(at.y / BOUND_STEP) * ((size.x + BOUND_STEP - 1u) / BOUND_STEP) + at.x / BOUND_STEP];
  }
  if (b.y > 0.0) {
    atomicMin(&slab_far, bitcast<u32>(b.x));
    atomicMax(&slab_near, bitcast<u32>(b.y));
  }
  workgroupBarrier();
  // The tile's whole slab is tested first. Where it lies off every map tile the marks cover,
  // none of the tile's pixels reads a tile that changed. A screen tile that a mark already
  // changed this frame needs no test.
  if (li == 0u) {
    let near = bitcast<f32>(atomicLoad(&slab_near));
    let rect = propagate_slab(tile * side, side, bitcast<f32>(atomicLoad(&slab_far)), near, size);
    let changed = tiles_age(tiles, changed_at[ti]) == 0u;
    slab_in_damage = !changed && near > 0.0 && propagate_in_damage(rect.xy, rect.zw);
  }
  if (!workgroupUniformLoad(&slab_in_damage)) {
    return;
  }
  let far = b.x;
  let near = b.y;
  if (!(near > 0.0)) {
    return;
  }
  let rect = propagate_slab(at, BOUND_STEP, far, near, size);
  let lo = rect.xy;
  let hi = rect.zw;
  if (!propagate_in_damage(lo, hi)) {
    return;
  }
  var hit = false;
  if (propagate_tiles(lo, hi) <= PROPAGATE_RECT_CAP) {
    hit = propagate_rect(lo, hi);
  } else {
    // A box that spans more than a few tiles comes from a square across an edge between near
    // and far, so each pixel is tested alone, at its own depth.
    for (var k = 0u; k < BOUND_STEP * BOUND_STEP; k++) {
      let p = at + vec2u(k % BOUND_STEP, k / BOUND_STEP);
      let v = propagate_depth(p, size);
      if (hit || !(v > 0.0)) {
        continue;
      }
      let t = propagate_texel(vec2f(p) + 0.5, v, size);
      hit = propagate_rect(t, t);
    }
  }
  if (hit) {
    changed_at[ti] = tiles.cnt.w;
  }
}
