// pxl-render/frame/shader/proxy_raster.wgsl

// The proxy's boxes drawn at the eye's rays' pixels (pxl-spec-core
// proxy.rs): each box an instance of a cube's 36 corners as `corner` winds
// them, the faces turned away culled, at its planes as the cut wrote them,
// grown already (`inflated`); a pixel keeps the nearest face its middle's
// ray meets, its box plus one and that face's depth. A box about the eye
// (`eyed`) is drawn nowhere: a reader walks it from the eye.

struct Raster {
  // The eye's unit right and the tangent of the screen's half-angle across,
  // its unit up and the one up, its unit forward and the near plane's
  // depth drawn nearer by the bias (`NEAR · (1 + BIAS)`).
  right: vec4f,
  up: vec4f,
  forward: vec4f,
  // 36 where the eye's axes turn the other way (`flipped`), each triangle
  // then wound the other way, else 0.
  flipped: vec4u,
};

@group(0) @binding(0) var<uniform> raster: Raster;
@group(0) @binding(1) var<storage, read> data: array<u32>;

struct RasterOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) index: u32,
};

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
  let p = mix(lo, hi, PROXY_CORNERS[raster.flipped.x + vi]);
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

// Its box plus one, and its depth as kept (`bound`), as f32 bits.
@fragment
fn fs(i: RasterOut) -> @location(0) vec4u {
  return vec4u(i.index, bitcast<u32>(i.clip.z), 0u, 0u);
}
