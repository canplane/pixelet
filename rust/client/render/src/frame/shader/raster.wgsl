// pxl-render/frame/shader/raster.wgsl

// The raster draws the voxels the eye and the light hold in their records as boxes, so that
// each drawn pixel keeps the number of the voxel its own ray enters first. pxl-engine raster.rs
// states the rule: which faces a box draws, the draw word, the reversed depth and the depth of
// a stale voxel. record_draw.wgsl writes the boxes, one word each, and the box's planes come
// from its node cache entry (record_box.wgsl `asset_box`). The cast reads the number each pixel
// keeps (cast.wgsl `cast_cs`).
//
// A box is one instance of three faces, those turned to the eye, two triangles each, wound as
// pxl-engine raster.rs `corner` winds them. A face keeps a fragment only where the pixel's ray
// enters the box across that face's axis (`raster_enters`). A ray enters a box through one
// face, so a kept fragment's depth is where its ray enters, and the early depth test keeps at
// each pixel the nearest box its ray enters. A box so near the eye that the near plane may cut
// it (`RASTER_EYED`) is drawn over the whole screen instead, and its fragments are tested
// without the axis.
//
// The light's samples' hits are drawn the same way into the light's box (pxl-engine param.rs
// `param::light::SAMPLE`), with an orthographic view, where `turn.y` is 1. A texel's ray
// starts at the texel's middle on the box's near plane and runs along the light. The depth is
// how far along the rays a point lies, as a share of their reach, and a nearer point has a
// greater depth. The light draws no box over the whole screen.

struct Raster {
  // The view's axes. For the eye, `xyz` holds its unit right, up and forward; `right.w` and
  // `up.w` hold the tangents of the screen's half-angles, and `forward.w` the near plane's
  // depth (`RASTER_NEAR`). For the light, `right` and `up` hold the box's steps across and
  // down a texel, each a unit vector with its length in `w`, and `forward` the rays'
  // direction.
  right: vec4f,
  up: vec4f,
  forward: vec4f,
  // `x` is 36 where the view's axes turn the other way from those `corner` winds front faces
  // for (pxl-engine raster.rs `is_flipped`), which selects the corners wound the other way,
  // and 0 otherwise. `y` is 1 for the light's view and 0 for the eye's.
  turn: vec4u,
  // `x` is how far a face grows (`GROWTH`): for the eye a tangent, which the vertex stage
  // multiplies by each corner's distance, and for the light a length. `y` and `z` are the
  // drawn pixels across and up, and `w` is how far the rays reach.
  screen: vec4f,
  // For the eye, where each pixel's ray passes through the pixel, as an offset from its
  // middle in drawn pixels (pxl-engine `eye::JITTER`). For the light, where the ray of box
  // texel (0, 0) starts, in units from `origin`.
  first: vec4f,
  // The Q3 position the boxes are laid from: the eye's origin, or where the ray of box texel
  // (0, 0) starts (pxl-engine `light::StartGrid::start`).
  origin: vec4i,
};

@group(0) @binding(1) var<uniform> raster: Raster;
/// The boxes to draw, one word each (pxl-engine `raster::draw_word`).
@group(0) @binding(2) var<storage, read> draws: array<u32>;
/// The node cache. Each box is one of its entries, which hold the record (pxl-engine `record`).
@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

// What a face passes to its fragments: its voxel's number, its box's planes, and the axis
// across which a pixel's ray must enter the box. A box drawn over the whole screen takes
// `RASTER_ANY`, which accepts any axis.
struct RasterOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) id: u32,
  @location(1) @interpolate(flat) lo: vec3f,
  @location(2) @interpolate(flat) hi: vec3f,
  @location(3) @interpolate(flat) axis: u32,
};

const RASTER_ANY: u32 = 3u;

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> RasterOut {
  let word = draws[ii];
  var o: RasterOut;
  o.id = word & RASTER_ID_MASK;
  // A face that is not drawn keeps every corner at this point, outside the clip volume.
  o.clip = vec4f(0.0, 0.0, -1.0, 1.0);
  let lacks = (o.id & RASTER_LACK) != 0u;
  let slot = (o.id & ~RASTER_LACK) - 1u;
  let origin = raster.origin.xyz;
  let b = asset_box(slot, entry_node(slot, origin), origin);
  let lo = b.lo;
  let hi = b.hi;
  o.lo = lo;
  o.hi = hi;
  // The faces the box may draw, one bit a side. The eye draws those an eye's ray asked about,
  // and the light draws every face some ray entered (pxl-engine `record::ASKED`,
  // `record::FACES`).
  let state = node_cache_word(state_at(slot));
  let faces = select((state & REC_ASKED) >> REC_ASKED_SHIFT, state & REC_FACES, raster.turn.y != 0u);
  if ((word & RASTER_EYED) != 0u) {
    o.axis = RASTER_ANY;
    if (vi < 3u) {
      // One triangle, wound counter-clockwise, covers the whole screen. The depth is reversed,
      // so a lack, at 1, lies before a hit, at `RASTER_EYED_DEPTH`.
      let xy = vec2f(f32((vi & 1u) * 4u) - 1.0, f32((vi >> 1u) * 4u) - 1.0);
      o.clip = vec4f(xy, select(RASTER_EYED_DEPTH, 1.0, lacks), 1.0);
    }
    return o;
  }
  let a = vi / 6u;
  o.axis = a;
  let light = raster.turn.y != 0u;
  // Of the two faces across `a`, only the one turned to the eye is drawn. For the eye that is
  // the high face where the box lies below the eye on `a`, the low face where it lies above,
  // and neither where the eye lies between the two planes. For the light it is the high face
  // where the rays run toward lower values on `a`, the low face where they run toward higher,
  // and neither where they run along the faces.
  var high = 0u;
  if (light) {
    let d = raster_moves(raster.forward[a]);
    if (d < 0.0) {
      high = 1u;
    } else if (!(d > 0.0)) {
      return o;
    }
  } else if (hi[a] < 0.0) {
    high = 1u;
  } else if (!(lo[a] > 0.0)) {
    return o;
  }
  // A hit draws the face only where its bit is set. Bit 2a + 1 is the low face, which a ray
  // running up the axis enters (pxl-engine `ray::face`), and bit 2a the high face. A lack
  // draws every face turned to the eye.
  if (!lacks && ((faces >> (2u * a + 1u - high)) & 1u) == 0u) {
    return o;
  }
  let c = raster_corner(raster.turn.x + (2u * a + high) * 6u + vi % 6u);
  let p0 = mix(lo, hi, c);
  // The face grows in its own plane, so the rasterizer's rounding cannot lose a pixel whose
  // middle lies on its edge. Its depth stays where a ray meets the face, and a fragment that
  // only the growth covers fails the test in `fs`.
  var g = (c * 2.0 - 1.0) * (raster.screen.x * select(length(p0), 1.0, light));
  g[a] = 0.0;
  let p = p0 + g;
  if (light) {
    // `i` and `j` place the corner on the light's box in texels across and down, with each
    // texel's middle at a whole number. The depth is 1 on the near plane and 0 at the rays'
    // reach.
    let q = p - raster.first.xyz;
    let i = dot(q, raster.right.xyz) / raster.right.w;
    let j = dot(q, raster.up.xyz) / raster.up.w;
    // The factor on the depth that puts a stale voxel behind every fresh one.
    let behind = select(1.0, RASTER_STALE_DEPTH, (word & RASTER_STALE) != 0u);
    o.clip = vec4f(
      (i + 0.5) / raster.screen.y * 2.0 - 1.0,
      1.0 - (j + 0.5) / raster.screen.z * 2.0,
      (1.0 - dot(q, raster.forward.xyz) / raster.screen.w) * behind,
      1.0,
    );
    return o;
  }
  // The eye's projection is the cast's (grid.wgsl `px_tan`) with a reversed, infinite depth:
  // a point at view depth w has depth RASTER_NEAR / w, which falls as the point lies farther
  // along a pixel's ray. The position is shifted against the jitter in `first`, so a pixel's
  // middle shows what its own jittered ray passes through.
  let depth = dot(p, raster.forward.xyz);
  let off = raster.first.xy / raster.screen.yz * vec2f(2.0, -2.0);
  o.clip = vec4f(
    dot(p, raster.right.xyz) / raster.right.w - off.x * depth,
    dot(p, raster.up.xyz) / raster.up.w - off.y * depth,
    raster.forward.w,
    depth,
  );
  return o;
}

// A ray's direction component as the cast holds it (pxl-engine cast.rs `Ray::new`): a
// component smaller than f32's least normal magnitude counts as 0.
fn raster_moves(d: f32) -> f32 {
  return select(d, 0.0, abs(d) < 1.17549435e-38);
}

// The ray's t at the near and the far plane of the slab `lo`..`hi` on one axis, counted from
// the ray's start, as pxl-engine cast.rs `plane_range` finds it. A ray that runs along the slab's
// planes lies inside it over its whole reach where it starts between them, and nowhere where
// it starts outside.
fn raster_plane_range(d: f32, lo: f32, hi: f32, far: f32) -> vec2f {
  if (d > 0.0) {
    let inv = 1.0 / d;
    return vec2f(lo * inv, hi * inv);
  }
  if (d < 0.0) {
    let inv = 1.0 / d;
    return vec2f(hi * inv, lo * inv);
  }
  if (lo <= 0.0 && 0.0 < hi) {
    return vec2f(0.0, far);
  }
  return vec2f(far, 0.0);
}

// The tangents across and up of the ray through the screen point `at`, in drawn pixels, formed
// as the cast forms them (px.wgsl `tan_at`) from the raster's view.
fn raster_tan(at: vec2f) -> vec2f {
  return tan_at(at, raster.screen.yz, vec2f(raster.right.w, raster.up.w));
}

// Whether the ray from `o` along the unit vector `u` enters the box `lo`..`hi` within its
// reach, and does so across axis `a` unless `a` is `RASTER_ANY`. The test is pxl-engine
// cast.rs `entry`'s: the ray's span inside the box is not empty, and the axis it enters across
// is the one whose near plane it reaches last among the axes it moves along, the lowest axis
// at a tie (`crossing`).
fn raster_enters(o: vec3f, u: vec3f, lo: vec3f, hi: vec3f, a: u32) -> bool {
  let d = vec3f(raster_moves(u.x), raster_moves(u.y), raster_moves(u.z));
  let far = raster.screen.w;
  let x = raster_plane_range(d.x, lo.x - o.x, hi.x - o.x, far);
  let y = raster_plane_range(d.y, lo.y - o.y, hi.y - o.y, far);
  let z = raster_plane_range(d.z, lo.z - o.z, hi.z - o.z, far);
  let t0 = max(max(max(0.0, x.x), y.x), z.x);
  let t1 = min(min(min(far, x.y), y.y), z.y);
  let near = vec3f(x.x, y.x, z.x);
  var e = 2u;
  if (d.x != 0.0) {
    e = 0u;
  } else if (d.y != 0.0) {
    e = 1u;
  }
  if (e < 1u && d.y != 0.0 && near.y > near[e]) {
    e = 1u;
  }
  if (e < 2u && d.z != 0.0 && near.z > near[e]) {
    e = 2u;
  }
  return t1 > t0 && (a == RASTER_ANY || e == a);
}

// A fragment keeps its voxel's number (pxl-engine raster.rs `id_of`) only where the pixel's
// own ray enters the box across this face (`raster_enters`). The eye's ray starts at the eye,
// with the direction pxl-engine cast.rs `through` forms. The light's ray starts at its texel's
// middle on the near plane and runs along the light.
@fragment
fn fs(i: RasterOut) -> @location(0) vec4u {
  var o = vec3f(0.0);
  var u = raster.forward.xyz;
  if (raster.turn.y != 0u) {
    let at = i.clip.xy - 0.5;
    o = raster.first.xyz + at.x * raster.right.w * raster.right.xyz + at.y * raster.up.w * raster.up.xyz;
  } else {
    let tan = raster_tan(i.clip.xy + raster.first.xy);
    let s = raster.forward.xyz + tan.x * raster.right.xyz + tan.y * raster.up.xyz;
    u = s / length(s);
  }
  if (!raster_enters(o, u, i.lo, i.hi, i.axis)) {
    discard;
  }
  return vec4u(i.id, 0u, 0u, 0u);
}
