// The screen pass's fragment stage. Each pixel shows the face its ray met, painted and lit, at
// a depth just behind the face (screen_ray.wgsl `px_ray`, `behind`).

@fragment
fn fs(v: VertexOut) -> FragOut {
  let p = px_ray(v.clip.xy);
  let a = p.a;
  let size = p.size;
  let d = p.d;
  let c = p.c;
  let faces = p.faces;
  let met = p.met;
  let h = hit_of(c.xy, d);
  // The derivatives are taken before any branch, because a derivative needs every pixel of the
  // quad running.
  let uv = box_uv(h);
  let ddx = dpdx(uv);
  let ddy = dpdy(uv);
  if (!met) {
    discard;
  }
  let painted = appearance(c);
  // The face's step across a pixel is found once, because both the lines' width and the
  // depth's step back read it.
  let steps = px_steps(a, size, h);
  let crease = creases(h, faces, steps);
  let s = Surface(
    painted.rgb,
    painted.layer,
    h.normal,
    crease.edge,
    h.at,
    uv,
    ddx,
    ddy,
    // The corners' occlusion. The screen's own occlusion is laid over the color later
    // (ao_apply.wgsl).
    crease.ao,
  );
  // The depth is written behind the face along the ray (`behind`), so what is laid on the face
  // wins the depth test.
  let clip = frame_view_proj() * vec4f(h.at * (1.0 + behind(h, steps)), 1.0);
  var o: FragOut;
  o.color = vec4f(tone(paint(s, seen_from_eye(h.at), light_of(h.at))), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}
