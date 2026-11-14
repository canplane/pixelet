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
  if (eye_word(EYE_STEPS) != 0u) {
    // The ray steps view (pxl-engine `eye::STEPS`). A ray that met nothing is drawn at the end
    // of what the eye draws, nearer than the far plane the sky stands at; one that met a face,
    // at the depth the face is drawn at (`behind`), so what is laid on the face shows as it does
    // there. Its color is the ray's steps, its third word in this view, on the steps' ramp, or
    // none where it took none (debug_look.json5).
    let at = select(d * bitcast<f32>(eye_word(EYE_FAR)), h.at * (1.0 + behind(h, px_steps(a, size, h))), met);
    let clip = frame_view_proj() * vec4f(at, 1.0);
    var o: FragOut;
    o.color = vec4f(select(steps_ramp(log2(1.0 + f32(c.z)) / log2(1.0 + STEPS_MOST)), STEPS_NONE, c.z == 0u), 1.0);
    o.depth = clip.z / clip.w;
    return o;
  }
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
  o.color = vec4f(tone(cell_share(paint(s, seen_from_eye(h.at), light_of(h.at)), c.x, h)), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}

// Under the camera gizmo (`frame_gizmo`), `color` tinted by the solid angle the face of the
// cell the ray stopped on shows the eye as the zoom rule weighs it: its cube's side at its
// zoom, `word`'s, squared over the distance squared, as a share of a grid cell's, `GRID_CELL`
// squared. A world's cell spans (1, `SPANS`] of the view's cells at its distance (pxl-engine
// grid.rs `cell_zoom_at`), so its share lies within (1, 4]: green there, blue finer and red
// coarser (debug_look.json5 `share`).
fn cell_share(color: vec3f, word: u32, h: Hit) -> vec3f {
  if (frame_gizmo() < 0.5) {
    return color;
  }
  let side = exp2(f32(i32(UNIT_ZOOM) - i32(ray_zoom(word))));
  let share = side * side / (dot(h.at, h.at) * GRID_CELL * GRID_CELL);
  let tint = select(select(SHARE_WITHIN, SHARE_FINER, share <= SHARE_FINER_MOST), SHARE_COARSER, share > SHARE_WITHIN_MOST);
  return mix(color, tint * 0.6, 0.45);
}
