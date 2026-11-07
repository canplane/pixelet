// pxl-render/frame/shader/screen_propagate.wgsl

// The light's changed tiles propagated to the screen (gpu/propagate.rs), the
// light's propagation turned round: where light_propagate.wgsl
// `light_propagate_cs`
// takes the screen's receivers to the light's tiles, this takes the light's
// tiles that changed back to the pixels that read them. A workgroup a screen
// tile, a thread a square of its pixels: the slab of the square's world,
// between the least and most depth the world's pass last left there, as the
// depth's bound keeps them (screen_bound.wgsl), the eye standing, taken into
// the light's map, a pixel at a time where the slab spans more than a few
// tiles, none where it lies off every tile the frame's marks cover, as the
// whole tile's slab is first; where any texel its shadow's taps read lies in a tile of the map
// that changed this frame (gpu/tiles.rs), the screen tile takes this frame's
// number, as a mark sets it (screen_mark.wgsl). The host reads nothing back:
// the frame it runs in is one whose drawn marks it counts. Bounds that
// cover: a tile too many costs, one missing is a failure.

// `Tiles`, `tiles_age` and `tiles_claimed` are tile_word.wgsl's (build.rs,
// word.rs `screen`).

struct Propagate {
  // From the screen's clip, its depth reversed, to units from the eye; and
  // from those to the light's clip.
  unproject: mat4x4f,
  light: mat4x4f,
  // The texels a receiver's taps reach each way, and the map's side.
  spread: vec4f,
};

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> propagate: Propagate;
@group(0) @binding(2) var<storage, read> light_changed_at: array<u32>;
@group(0) @binding(3) var<uniform> light_tiles: Tiles;
@group(0) @binding(4) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(5) var<uniform> tiles: Tiles;
// Each square's least and most depth drawn, `BOUND_STEP` pixels a side, a
// thread's of a screen tile's `BOUND_SQUARES` a side (constant.wgsl), as
// the last frame's world's pass left them (screen_bound.wgsl).
@group(0) @binding(6) var<storage, read> bounds: array<vec2f>;
// The map's tiles the frame's marks cover, from the first across and down to
// past the last, as the host handed them and the drawn marks widened them
// (gpu/tiles.rs `Tiles::marks_box`, tile_mark.wgsl): none changed off them.
@group(0) @binding(7) var<storage, read> marks_box: vec4u;

// Whether tile `t` of the map changed this frame: claimed by its words.
fn propagate_changed(t: vec2i) -> bool {
  let n = vec2i(light_tiles.count.xy);
  if (any(t < vec2i(0)) || any(t >= n)) {
    return false;
  }
  let ii = u32(t.y * n.x + t.x);
  return tiles_claimed(light_tiles, light_changed_at[ii]);
}

// The depth the world's pass left at pixel `p` of a target `size`, 0 past
// it.
fn propagate_depth(p: vec2u, size: vec2u) -> f32 {
  if (any(p >= size)) {
    return 0.0;
  }
  return textureLoad(depth, vec2i(p), 0);
}

// Where the pixel point `c`, in pixels of a target `size`, at depth `d`
// (reversed), lies in the light's map, in its texels.
fn propagate_texel(c: vec2f, d: f32, size: vec2u) -> vec2f {
  let ndc = vec2f(c.x / f32(size.x) * 2.0 - 1.0, 1.0 - c.y / f32(size.y) * 2.0);
  let e = propagate.unproject * vec4f(ndc, d, 1.0);
  let l = propagate.light * vec4f(e.xyz / e.w, 1.0);
  return vec2f(l.x * 0.5 + 0.5, 0.5 - l.y * 0.5) * propagate.spread.y;
}

// The tiles of the map a receiver's taps read about the texels from `lo` to
// `hi`, held to a tile past the map each way.
fn propagate_range(lo: vec2f, hi: vec2f) -> vec4i {
  let n = vec2i(light_tiles.count.xy);
  let side = light_tiles.size.w;
  // Clamped as integers, so a range lies within a tile past the map however its floats came.
  let a = clamp(vec2i(floor((lo - propagate.spread.x) / side)), vec2i(-1), n);
  let b = clamp(vec2i(floor((hi + propagate.spread.x) / side)), vec2i(-1), n);
  return vec4i(a, b);
}

// Whether any tile `propagate_range` holds lies among those the marks cover.
fn propagate_in_marks_box(lo: vec2f, hi: vec2f) -> bool {
  let r = propagate_range(lo, hi);
  let m = vec4i(marks_box);
  return r.x < m.z && r.z >= m.x && r.y < m.w && r.w >= m.y;
}

// The box in the map's texels of the slab of pixels from `at` to `at + side`
// each way between depths `far` and `near` (reversed), its 8 corners taken
// into the map: what the pixels between read lies within it.
fn propagate_slab(at: vec2u, side: u32, far: f32, near: f32, size: vec2u) -> array<vec2f, 2> {
  var lo = vec2f(1e30);
  var hi = vec2f(-1e30);
  for (var k = 0u; k < 8u; k++) {
    let c = at + vec2u(k & 1u, (k >> 1u) & 1u) * side;
    let t = propagate_texel(vec2f(c), select(far, near, (k & 4u) != 0u), size);
    lo = min(lo, t);
    hi = max(hi, t);
  }
  return array<vec2f, 2>(lo, hi);
}

// The tile's least and most depth drawn, as its squares' bounds' bits (a
// depth is positive, so its bits keep its order), and whether its slab
// lies on a tile the marks cover.
var<workgroup> slab_far: atomic<u32>;
var<workgroup> slab_near: atomic<u32>;
var<workgroup> slab_in_marks_box: bool;

// How many tiles `propagate_range` holds.
fn propagate_tiles(lo: vec2f, hi: vec2f) -> i32 {
  let r = propagate_range(lo, hi);
  return (r.z - r.x + 1) * (r.w - r.y + 1);
}

// Whether any tile `propagate_range` holds changed this frame.
fn propagate_box(lo: vec2f, hi: vec2f) -> bool {
  let r = propagate_range(lo, hi);
  var hit = false;
  for (var y = r.y; y <= r.w; y++) {
    for (var x = r.x; x <= r.z; x++) {
      hit = hit || propagate_changed(vec2i(x, y));
    }
  }
  return hit;
}

// The tiles a square's box asks at most before each pixel is asked alone.
const PROPAGATE_BOX_MOST: i32 = 16;

@compute @workgroup_size(BOUND_SQUARES, BOUND_SQUARES, 1)
fn screen_propagate_cs(
  @builtin(workgroup_id) wg: vec3u,
  @builtin(local_invocation_id) l: vec3u,
  @builtin(local_invocation_index) li: u32,
) {
  let tile = wg.xy;
  if (any(tile >= tiles.count.xy)) {
    return;
  }
  let ti = tile.y * tiles.count.x + tile.x;
  let size = vec2u(textureDimensions(depth));
  let side = u32(tiles.size.w);
  let at = tile * side + l.xy * BOUND_STEP;
  if (li == 0u) {
    atomicStore(&slab_far, bitcast<u32>(2.0));
    atomicStore(&slab_near, 0u);
  }
  workgroupBarrier();
  // The square's least and most depth drawn, kept; the sky, which no shadow
  // falls on, left out (a square of sky alone holds 2 and 0).
  var b = vec2f(2.0, 0.0);
  if (all(at < size)) {
    b = bounds[(at.y / BOUND_STEP) * ((size.x + BOUND_STEP - 1u) / BOUND_STEP) + at.x / BOUND_STEP];
  }
  if (b.y > 0.0) {
    atomicMin(&slab_far, bitcast<u32>(b.x));
    atomicMax(&slab_near, bitcast<u32>(b.y));
  }
  workgroupBarrier();
  // The tile's slab first: off every tile the marks cover, none of its
  // pixels reads a tile that changed. None where a mark changed it this
  // frame already.
  if (li == 0u) {
    let near = bitcast<f32>(atomicLoad(&slab_near));
    let box = propagate_slab(tile * side, side, bitcast<f32>(atomicLoad(&slab_far)), near, size);
    let changed = tiles_age(tiles, changed_at[ti]) == 0u;
    slab_in_marks_box = !changed && near > 0.0 && propagate_in_marks_box(box[0], box[1]);
  }
  if (!workgroupUniformLoad(&slab_in_marks_box)) {
    return;
  }
  let far = b.x;
  let near = b.y;
  if (!(near > 0.0)) {
    return;
  }
  // The square's slab: what its pixels read lies within its box.
  let box = propagate_slab(at, BOUND_STEP, far, near, size);
  let lo = box[0];
  let hi = box[1];
  if (!propagate_in_marks_box(lo, hi)) {
    return;
  }
  var hit = false;
  if (propagate_tiles(lo, hi) <= PROPAGATE_BOX_MOST) {
    hit = propagate_box(lo, hi);
  } else {
    // A box past a few tiles (an edge between near and far): each pixel
    // its own, as tight as before.
    for (var k = 0u; k < BOUND_STEP * BOUND_STEP; k++) {
      let p = at + vec2u(k % BOUND_STEP, k / BOUND_STEP);
      let v = propagate_depth(p, size);
      if (hit || !(v > 0.0)) {
        continue;
      }
      let t = propagate_texel(vec2f(p) + 0.5, v, size);
      hit = propagate_box(t, t);
    }
  }
  if (hit) {
    changed_at[ti] = tiles.count.w;
  }
}
