// pxl-render/frame/shader/screen_reach.wgsl

// The light's changed tiles reached across to the screen (gpu/reach.rs), the
// light's reach turned round: where pxl-shader light.rs `light_reach_cs`
// takes the screen's receivers to the light's tiles, this takes the light's
// tiles that changed back to the pixels that read them. A workgroup a screen
// tile, a thread a square of its pixels: the slab of the square's world,
// between the least and most depth the world's pass last left there, as the
// depth's bound keeps them (screen_bound.wgsl), the eye standing, taken into
// the light's map, a pixel at a time where the slab spans more than a few
// tiles, none where it lies off every tile the frame's marks cover, as the
// whole tile's slab is first; where any texel its shadow's taps read lies in a tile of the map
// that changed this frame (gpu/tiles.rs), the screen tile takes this frame's
// number, as a mark sets it (screen_mark.wgsl), and the frame's number is
// kept in `reached` where it is the latest, which the host reads back to know
// whether any tile gathers. Bounds that cover: a tile too many costs, one
// missing is a failure.

struct Tiles {
  count: vec4u,
  size: vec4f,
};

struct Reach {
  // From the screen's clip, its depth reversed, to units from the eye; and
  // from those to the light's clip.
  unproject: mat4x4f,
  light: mat4x4f,
  // The texels a receiver's taps reach each way, and the map's side.
  spread: vec4f,
  // The map's tiles the frame's marks cover, from the first across and down
  // to past the last (gpu/tiles.rs `Tiles::marks_box`): none changed off
  // them.
  marks_box: vec4f,
};

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> reach: Reach;
@group(0) @binding(2) var<storage, read> light_changed_at: array<u32>;
@group(0) @binding(3) var<uniform> light_tiles: Tiles;
@group(0) @binding(4) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(5) var<uniform> tiles: Tiles;
// The number of the last frame the reach marked a tile in.
@group(0) @binding(6) var<storage, read_write> reached: atomic<u32>;
// Each square's least and most depth drawn, `REACH_STEP` pixels a side, as
// the last frame's world's pass left them (screen_bound.wgsl).
@group(0) @binding(7) var<storage, read> bounds: array<vec2f>;

// The pixels a side of a thread's square of a screen tile, eight threads a
// side (pxl-spec-core `screen::TILE`, 32).
const REACH_STEP: u32 = 4u;

// Whether tile `t` of the map changed this frame: claimed by its words.
fn reach_changed(t: vec2i) -> bool {
  let n = vec2i(light_tiles.count.xy);
  if (any(t < vec2i(0)) || any(t >= n)) {
    return false;
  }
  let ii = u32(t.y * n.x + t.x);
  return light_tiles.count.w - light_changed_at[ii] < light_tiles.count.z;
}

// The depth the world's pass left at pixel `p` of a target `size`, 0 past
// it.
fn reach_depth(p: vec2u, size: vec2u) -> f32 {
  if (any(p >= size)) {
    return 0.0;
  }
  return textureLoad(depth, vec2i(p), 0);
}

// Where the pixel point `c`, in pixels of a target `size`, at depth `d`
// (reversed), lies in the light's map, in its texels.
fn reach_texel(c: vec2f, d: f32, size: vec2u) -> vec2f {
  let ndc = vec2f(c.x / f32(size.x) * 2.0 - 1.0, 1.0 - c.y / f32(size.y) * 2.0);
  let e = reach.unproject * vec4f(ndc, d, 1.0);
  let l = reach.light * vec4f(e.xyz / e.w, 1.0);
  return vec2f(l.x * 0.5 + 0.5, 0.5 - l.y * 0.5) * reach.spread.y;
}

// The tiles of the map a receiver's taps read about the texels from `lo` to
// `hi`, held to a tile past the map each way.
fn reach_range(lo: vec2f, hi: vec2f) -> vec4i {
  let n = vec2f(light_tiles.count.xy);
  let side = light_tiles.size.w;
  let a = vec2i(clamp(floor((lo - reach.spread.x) / side), vec2f(-1.0), n));
  let b = vec2i(clamp(floor((hi + reach.spread.x) / side), vec2f(-1.0), n));
  return vec4i(a, b);
}

// Whether any tile `reach_range` holds lies among those the marks cover.
fn reach_in_marks_box(lo: vec2f, hi: vec2f) -> bool {
  let r = reach_range(lo, hi);
  let m = vec4i(reach.marks_box);
  return r.x < m.z && r.z >= m.x && r.y < m.w && r.w >= m.y;
}

// The box in the map's texels of the slab of pixels from `at` to `at + side`
// each way between depths `far` and `near` (reversed), its 8 corners taken
// into the map: what the pixels between read lies within it.
fn reach_slab(at: vec2u, side: u32, far: f32, near: f32, size: vec2u) -> array<vec2f, 2> {
  var lo = vec2f(1e30);
  var hi = vec2f(-1e30);
  for (var k = 0u; k < 8u; k++) {
    let c = at + vec2u(k & 1u, (k >> 1u) & 1u) * side;
    let t = reach_texel(vec2f(c), select(far, near, (k & 4u) != 0u), size);
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

// How many tiles `reach_range` holds.
fn reach_tiles(lo: vec2f, hi: vec2f) -> i32 {
  let r = reach_range(lo, hi);
  return (r.z - r.x + 1) * (r.w - r.y + 1);
}

// Whether any tile `reach_range` holds changed this frame.
fn reach_box(lo: vec2f, hi: vec2f) -> bool {
  let r = reach_range(lo, hi);
  var hit = false;
  for (var y = r.y; y <= r.w; y++) {
    for (var x = r.x; x <= r.z; x++) {
      hit = hit || reach_changed(vec2i(x, y));
    }
  }
  return hit;
}

// The tiles a square's box asks at most before each pixel is asked alone.
const REACH_BOX_MOST: i32 = 16;

@compute @workgroup_size(8, 8, 1)
fn screen_reach_cs(
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
  let at = tile * side + l.xy * REACH_STEP;
  if (li == 0u) {
    atomicStore(&slab_far, bitcast<u32>(2.0));
    atomicStore(&slab_near, 0u);
  }
  workgroupBarrier();
  // The square's least and most depth drawn, kept; the sky, which no shadow
  // falls on, left out (a square of sky alone holds 2 and 0).
  var b = vec2f(2.0, 0.0);
  if (all(at < size)) {
    b = bounds[(at.y / REACH_STEP) * ((size.x + REACH_STEP - 1u) / REACH_STEP) + at.x / REACH_STEP];
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
    let box = reach_slab(tile * side, side, bitcast<f32>(atomicLoad(&slab_far)), near, size);
    let changed = tiles.count.w - changed_at[ti] == 0u;
    slab_in_marks_box = !changed && near > 0.0 && reach_in_marks_box(box[0], box[1]);
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
  let box = reach_slab(at, REACH_STEP, far, near, size);
  let lo = box[0];
  let hi = box[1];
  if (!reach_in_marks_box(lo, hi)) {
    return;
  }
  var hit = false;
  if (reach_tiles(lo, hi) <= REACH_BOX_MOST) {
    hit = reach_box(lo, hi);
  } else {
    // A box past a few tiles (an edge between near and far): each pixel
    // its own, as tight as before.
    for (var k = 0u; k < REACH_STEP * REACH_STEP; k++) {
      let p = at + vec2u(k % REACH_STEP, k / REACH_STEP);
      let v = reach_depth(p, size);
      if (hit || !(v > 0.0)) {
        continue;
      }
      let t = reach_texel(vec2f(p) + 0.5, v, size);
      hit = reach_box(t, t);
    }
  }
  if (hit) {
    changed_at[ti] = tiles.count.w;
    atomicMax(&reached, tiles.count.w);
  }
}
