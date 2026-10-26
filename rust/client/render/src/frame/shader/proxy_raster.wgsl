// pxl-render/frame/shader/proxy_raster.wgsl

// The proxy's boxes drawn at the eye's rays' pixels (pxl-spec-core
// proxy.rs): each box an instance of a cube's 36 corners, at its planes as
// the cut wrote them, grown already (`inflated`); a pixel keeps the
// nearest face its middle's ray meets, and its box, plus one. A box about
// the eye (`eyed`) is drawn nowhere: a reader walks it from the eye.

struct Raster {
  // The eye's unit right and the tangent of the screen's half-angle across,
  // its unit up and the one up, its unit forward and the near plane's
  // depth drawn nearer by the bias (`NEAR · (1 + BIAS)`).
  right: vec4f,
  up: vec4f,
  forward: vec4f,
};

@group(0) @binding(0) var<uniform> raster: Raster;
@group(0) @binding(1) var<storage, read> data: array<u32>;

struct RasterOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) index: u32,
};

// A cube's corner `k` of its 36: two triangles a face, the faces in any
// order, as both sides are drawn.
fn corner(k: u32) -> vec3f {
  let face = k / 6u;
  var quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
  let t = quad[k % 6u];
  let a = face / 2u;
  let s = f32(face % 2u);
  let u = f32(t == 1u || t == 2u);
  let v = f32(t >= 2u);
  var c = vec3f(0.0);
  c[a] = s;
  c[(a + 1u) % 3u] = u;
  c[(a + 2u) % 3u] = v;
  return c;
}

fn plane(w: u32) -> f32 {
  return bitcast<f32>(data[w]);
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> RasterOut {
  let w = PROXY_BOX_AT + ii * PROXY_BOX_WORDS;
  var o: RasterOut;
  o.index = ii + 1u;
  if ((data[w + PROXY_BOX_FLAGS] & PROXY_EYED) != 0u) {
    o.clip = vec4f(0.0, 0.0, -1.0, 1.0);
    return o;
  }
  let lo = vec3f(plane(w + PROXY_BOX_LO), plane(w + PROXY_BOX_LO + 1u), plane(w + PROXY_BOX_LO + 2u));
  let hi = vec3f(plane(w + PROXY_BOX_HI), plane(w + PROXY_BOX_HI + 1u), plane(w + PROXY_BOX_HI + 2u));
  let p = mix(lo, hi, corner(vi));
  // The cast's projection (pxl-shader cast.rs `pixel_tan`), reversed and
  // infinite: depth NEAR·(1 + BIAS) over the view depth.
  o.clip = vec4f(
    dot(p, raster.right.xyz) / raster.right.w,
    dot(p, raster.up.xyz) / raster.up.w,
    raster.forward.w,
    dot(p, raster.forward.xyz),
  );
  return o;
}

@fragment
fn fs(i: RasterOut) -> @location(0) vec4u {
  return vec4u(i.index, 0u, 0u, 0u);
}
