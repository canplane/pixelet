// pxl-render/decor_cnt.wgsl

// Picks the region each of the eye's records draws in this frame, writes it into the record's
// first word, and counts it. A workgroup takes an entry's records or a run of placed decor. A
// record draws nothing where the rays at its distance ask another zoom, held to the clip levels
// that hold its column (`generation::ray_zoom`), than the one its record's clip level draws at
// (`generation::drawn_at`), or where it is the decor left out. Otherwise it is sized for the
// screen as a model, an impostor, or culled (`generation::drawn`).

@group(0) @binding(9) var<storage, read_write> recs: array<u32>;
@group(0) @binding(13) var<storage, read> drawing: array<u32>;
@group(0) @binding(14) var<storage, read_write> regions: array<atomic<u32>>;

/// `generation::dist2`: the squared distance from `origin` to decor standing at `at`,
/// measured from the middle of its cell's base. X goes round the world as the steps wrap.
fn dist2(origin: vec3<i32>, at: vec3<i32>) -> f32 {
  let half = 1i << (FRAC_W - 1u);
  let steps = (at << vec3<u32>(FRAC_W)) + vec3<i32>(half, half, 0i) - origin;
  let d = vec3<f32>(steps) * (1.0 / f32(1u << FRAC_W));
  return d.x * d.x + d.y * d.y + d.z * d.z;
}

/// `generation::ray_zoom`: the zoom asked, `ask_zoom`, held to what the clip levels hold at
/// leaf cell `c`.
fn ray_zoom(ask_zoom: u32, c: vec2<i32>) -> u32 {
  let z = clip_zoom(ask_zoom);
  let s = LEAF_CELL_ZOOM - z;
  let g = vec2<i32>(wrap_idx(z, c.x >> s), c.y >> s);
  return min(ask_zoom, lookup_from(step_to(LOOKUP_NONE, z, g).place));
}

fn cnt_one(i: u32) {
  let w = recs[i];
  if (w == UNGENERATED) {
    return;
  }
  let corner = vec3<i32>(i32(recs[i + 1u]), i32(recs[i + 2u]), i32(recs[i + 3u]));
  let origin = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let d2 = dist2(origin, corner);
  let placed = i >= PLACED_HEADER + PLACED_HEADER_WORDS;
  let ask_zoom = zoom_at(d2, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
  let zoom = select(ray_zoom(ask_zoom, corner.xy), ask_zoom, placed);
  let cell_zoom = rec_cell_zoom(w);
  var drawn_here = zoom == cell_zoom;
  if (placed) {
    drawn_here = zoom >= cell_zoom;
  } else if (cell_zoom >= LEAF_CELL_ZOOM) {
    drawn_here = zoom >= LEAF_CELL_ZOOM;
  }
  let group = decor_first_group(rec_decor(w)) + rec_variant(w);
  let hidden = drawing[DRAWING_HIDE + 3u] != 0u
    && i32(drawing[DRAWING_HIDE]) == corner.x
    && i32(drawing[DRAWING_HIDE + 1u]) == corner.y
    && i32(drawing[DRAWING_HIDE + 2u]) == corner.z;
  let across = bitcast<f32>(drawing[DRAWING_EXTENTS + group]) / max(sqrt(d2), NEAREST_U);
  var lod = DRAWING_CULLED;
  if (across >= bitcast<f32>(drawing[DRAWING_MODEL_ANGLE])) {
    lod = DRAWING_MODEL;
  } else if (across >= bitcast<f32>(drawing[DRAWING_IMPOSTOR_ANGLE])) {
    lod = DRAWING_IMPOSTOR;
  }
  let region = select(2u * group + lod, DRAWING_NONE, hidden || !drawn_here || lod == DRAWING_CULLED);
  recs[i] = (w & ((1u << DRAWING_REGION_SHIFT) - 1u)) | (region << DRAWING_REGION_SHIFT);
  if (region != DRAWING_NONE) {
    atomicAdd(&regions[region], 1u);
  }
}

@compute @workgroup_size(LANES)
fn decor_cnt_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  if (!holds_recs(group.x)) {
    return;
  }
  for (var k = lane.x; k < RUNS; k += LANES) {
    let run = run_of(group.x, k);
    for (var n = 0u; n < run.y; n += 1u) {
      cnt_one(rec_at(run.x + n));
    }
  }
}
