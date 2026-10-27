// The screen pass's lit pixel: the face its ray met, painted and lit, at
// the depth `behind` it (screen_ray.wgsl `pixel_ray`).

@fragment
fn fs(v: VertexOut) -> FragOut {
  let p = pixel_ray(v.clip.xy);
  let a = p.a;
  let size = p.size;
  let d = p.d;
  let c = p.c;
  let faces = p.faces;
  let met = p.met;
  let h = p.h;
  // Taken before any branch: a derivative needs the quad whole.
  let uv = box_uv(h);
  let ddx = dpdx(uv);
  let ddy = dpdy(uv);
  if (eye_word(EYE_STEPS) != 0u) {
    // A ray that met nothing at the end of what the eye observes, nearer than
    // the far plane's 0 the sky stands at; one that met a face at the depth
    // the lit pixel is drawn at (`behind`), so what is laid on the face shows
    // as it does there.
    let far = d * bitcast<f32>(eye_word(EYE_REACH));
    let at = select(far, h.at * (1.0 + behind(h, pixel_steps(a, size, h))), met);
    let clip = frame_view_proj() * vec4f(at, 1.0);
    var o: FragOut;
    // The nodes asked, the third word in this view; none where the pixel
    // showed a sample's block and walked nothing, grey.
    o.color = vec4f(select(turbo(log2(1.0 + f32(c.z)) / log2(1.0 + STEPS_MOST)), vec3f(STEPS_NONE), c.z == 0u), 1.0);
    o.depth = clip.z / clip.w;
    return o;
  }
  if (!met) {
    discard;
  }
  let painted = drawn(c);
  // The face's step across a pixel, once: the lines' width and the depth's
  // step back both read it.
  let steps = pixel_steps(a, size, h);
  let crease = creases(h, faces, steps);
  let s = Surface(
    painted.rgb,
    painted.taps,
    h.normal,
    crease.edge,
    h.at,
    uv,
    ddx,
    ddy,
    screen_open(v.clip.xy) * crease.open,
    seen_from_eye(h.at),
  );
  // The depth `behind` the face along the ray, so what is laid on it wins.
  let clip = frame_view_proj() * vec4f(h.at * (1.0 + behind(h, steps)), 1.0);
  var o: FragOut;
  o.color = vec4f(finish(cell_share(paint(s), c.x, h), v.clip.xy), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}

// Under the camera gizmo: the solid angle the face of the block the ray
// stopped on shows the eye, its cube's side at its zoom squared over the
// distance squared, foreshortened by the face's cosine to the ray, as a
// share of a grid cell's, `frame_cell` squared. The zoom rule weighs the
// side over the distance alone, so a top seen at a grazing angle reads
// finer than its threshold.
//
// 10-12 tinted the chunks' faces by the same share of its own cell, τ/96
// (96 sides its split distance), green within (¼, 1], blue finer, red
// coarser: a block there spans (½, 1] of that cell. Today's cell is
// τ/`SPLIT`, τ/192, half as wide at the same zoom rule (pxl-spec-core
// grid.rs `cell_at`: a block spans (1, 2] cells), so that cell squared is 4
// of today's: within (1, 4] green, finer blue, coarser red.
fn cell_share(color: vec3f, word: u32, h: Hit) -> vec3f {
  if (frame_gizmo() < 0.5) {
    return color;
  }
  let side = exp2(f32(i32(UNIT_ZOOM) - i32(ray_zoom(word))));
  let r2 = dot(h.at, h.at);
  let seen = side * side * abs(dot(h.normal, h.at)) / (r2 * sqrt(r2));
  let steps = log2(seen / (frame_cell() * frame_cell()));
  let tint = select(select(vec3f(0.1, 0.9, 0.2), vec3f(0.1, 0.3, 1.0), steps <= 0.0), vec3f(1.0, 0.15, 0.1), steps > 2.0);
  return mix(color, tint * 0.6, 0.45);
}
