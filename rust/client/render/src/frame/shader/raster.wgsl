// pxl-render/frame/shader/raster.wgsl

// The samples' stops drawn at the eye's rays' pixels (pxl-spec-core
// raster.rs): each box an instance, its three faces turned to the eye, one
// an axis, as `corner` winds them, the faces turned away never drawn, each
// grown in its own plane by `GROWN` of a drawn pixel. A hit's box draws the
// face across its sample's axis alone, a lack's every face turned to the
// eye. A face keeps a pixel only where the pixel's own ray enters the box
// across that face's axis (`raster_enters`). A ray enters a box through one
// face, and the face lies in its plane, so a kept fragment's depth is where
// the ray enters the box: the depth test, early, keeps the nearest box the
// pixel's ray enters of every sample's. A pixel keeps that face's sample's
// number, which the cast's pixels read (pxl-shader cast.rs `cast_cs`). A box
// about the eye (`RASTER_EYED`), which the near plane may cut, is drawn over
// the whole screen at the nearest depth, a lack's nearer still, tested alike
// but for the axis.

struct Raster {
  // The eye's unit right and the tangent of the screen's half-angle across,
  // its unit up and the one up, its unit forward and the near plane's
  // depth (`RASTER_NEAR`).
  right: vec4f,
  up: vec4f,
  forward: vec4f,
  // 36 where the eye's axes turn the other way (`flipped`), each triangle
  // then wound the other way, else 0.
  turn: vec4u,
  // The angle a face grows by, a share of a corner's distance; the drawn
  // pixels across and up; how far the eye's rays reach.
  screen: vec4f,
};

@group(0) @binding(0) var<uniform> raster: Raster;
@group(0) @binding(1) var<storage, read> boxes: array<u32>;

// A face's sample's number; its box's planes; and the axis a pixel's ray
// must enter it across, `RASTER_ANY` for a box about the eye.
struct RasterOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) id: u32,
  @location(1) @interpolate(flat) lo: vec3f,
  @location(2) @interpolate(flat) hi: vec3f,
  @location(3) @interpolate(flat) axis: u32,
};

const RASTER_ANY: u32 = 3u;

fn raster_plane(w: u32) -> f32 {
  return bitcast<f32>(boxes[w]);
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> RasterOut {
  let w = ii * RASTER_BOX_WORDS;
  var o: RasterOut;
  o.id = boxes[w + RASTER_BOX_ID];
  // Past the near plane: a corner of a face not drawn.
  o.clip = vec4f(0.0, 0.0, -1.0, 1.0);
  let word = boxes[w + RASTER_BOX_AXIS];
  let lacks = (o.id & RASTER_LACK) != 0u;
  let lo = vec3f(raster_plane(w + RASTER_BOX_LO), raster_plane(w + RASTER_BOX_LO + 1u), raster_plane(w + RASTER_BOX_LO + 2u));
  let hi = vec3f(raster_plane(w + RASTER_BOX_HI), raster_plane(w + RASTER_BOX_HI + 1u), raster_plane(w + RASTER_BOX_HI + 2u));
  o.lo = lo;
  o.hi = hi;
  if ((word & RASTER_EYED) != 0u) {
    o.axis = RASTER_ANY;
    if (vi < 3u) {
      // Counter-clockwise over the whole screen.
      let xy = vec2f(f32((vi & 1u) * 4u) - 1.0, f32((vi >> 1u) * 4u) - 1.0);
      o.clip = vec4f(xy, select(RASTER_EYED_DEPTH, 1.0, lacks), 1.0);
    }
    return o;
  }
  let a = vi / 6u;
  o.axis = a;
  if (!lacks && a != (word & RASTER_AXIS_MASK)) {
    return o;
  }
  // The face across `a` turned to the eye: the high one where the eye is
  // past it, the low where before; none where the eye is between them.
  var high = 0u;
  if (hi[a] < 0.0) {
    high = 1u;
  } else if (!(lo[a] > 0.0)) {
    return o;
  }
  let c = RASTER_CORNERS[raster.turn.x + (2u * a + high) * 6u + vi % 6u];
  let p0 = mix(lo, hi, c);
  // Grown in its own plane, so a pixel's middle on its edge is not lost to
  // the rasterizer's rounding: its depth stays where a ray meets the face,
  // and what the growth alone covers fails the fragment's test.
  var g = (c * 2.0 - 1.0) * (raster.screen.x * length(p0));
  g[a] = 0.0;
  let p = p0 + g;
  // The cast's projection (pxl-shader cast.rs `pixel_tan`), reversed and
  // infinite: depth RASTER_NEAR over the view depth, which along a pixel's
  // ray falls as the ray's distance grows.
  o.clip = vec4f(
    dot(p, raster.right.xyz) / raster.right.w,
    dot(p, raster.up.xyz) / raster.up.w,
    raster.forward.w,
    dot(p, raster.forward.xyz),
  );
  return o;
}

// A ray's direction component as the cast holds it (pxl-spec-core cast.rs
// `Ray::new`): 0 under f32's least normal magnitude.
fn raster_moves(d: f32) -> f32 {
  return select(d, 0.0, abs(d) < 1.17549435e-38);
}

// The ray's t at the near and far planes on one axis (pxl-spec-core cast.rs
// `planes`, from 0): an axis it runs beside, its whole reach inside the
// slab, none outside.
fn raster_planes(d: f32, lo: f32, hi: f32, reach: f32) -> vec2f {
  if (d > 0.0) {
    let inv = 1.0 / d;
    return vec2f(lo * inv, hi * inv);
  }
  if (d < 0.0) {
    let inv = 1.0 / d;
    return vec2f(hi * inv, lo * inv);
  }
  if (lo <= 0.0 && 0.0 < hi) {
    return vec2f(0.0, reach);
  }
  return vec2f(reach, 0.0);
}

// The drawn pixel's tangents across and up at its middle `at`, as
// pxl-shader cast.rs `pixel_tan` forms them.
fn raster_tan(at: vec2f) -> vec2f {
  let ndc_x = at.x / raster.screen.y * 2.0 - 1.0;
  let ndc_y = 1.0 - at.y / raster.screen.z * 2.0;
  return vec2f(ndc_x * raster.right.w, ndc_y * raster.up.w);
}

// Whether the ray at the tangents `tan` enters the box `lo`..`hi` within
// its reach, across axis `a` unless `RASTER_ANY`: pxl-spec-core cast.rs
// `entry`, its span in the box not empty, the axis entered the last of the
// near planes it reaches among those it moves along, the lowest at a tie
// (`crossing`).
fn raster_enters(tan: vec2f, lo: vec3f, hi: vec3f, a: u32) -> bool {
  // Its direction, as pxl-spec-core cast.rs `through`.
  let s = raster.forward.xyz + tan.x * raster.right.xyz + tan.y * raster.up.xyz;
  let u = s / sqrt(dot(s, s));
  let d = vec3f(raster_moves(u.x), raster_moves(u.y), raster_moves(u.z));
  let reach = raster.screen.w;
  let x = raster_planes(d.x, lo.x, hi.x, reach);
  let y = raster_planes(d.y, lo.y, hi.y, reach);
  let z = raster_planes(d.z, lo.z, hi.z, reach);
  let t0 = max(max(max(0.0, x.x), y.x), z.x);
  let t1 = min(min(min(reach, x.y), y.y), z.y);
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

// Its sample's number (pxl-spec-core raster.rs `id_of`) where the pixel's
// ray enters its box across this face (`raster_enters`).
@fragment
fn fs(i: RasterOut) -> @location(0) vec4u {
  if (!raster_enters(raster_tan(i.clip.xy), i.lo, i.hi, i.axis)) {
    discard;
  }
  return vec4u(i.id, 0u, 0u, 0u);
}
