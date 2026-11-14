// pxl-render/frame/shader/light_mark_draws.wgsl

// Stands the drawn receivers of the shadow map that the eye sees, by the world's one rule
// (light_mark.wgsl, `Light::box_tiles`). The host's receivers come first, one workgroup for
// each: what the host draws that reads the map, standing within the box its draw recorded
// (`light::RECEIVER_BOXES_AT`). They are the draws gpu/draw.rs `DrawList` records, at most
// `light::RECEIVER_BOX_CAP`. The decor generated comes after (gpu/generator.rs), one thread for
// each slot of its instances that is placed, standing where decor_compact.wgsl lays it. The
// cast's one thread opens the workgroups in a buffer of their own, which this pass reads its
// count from and does not bind (light_mark_draws_open.wgsl, `light::DRAWS_DISPATCH_WORDS`). A
// receiver is seen where a ball about its box meets the view (`eye::sees`). A placed decor's
// ball lies about its middle and is as wide as decor is tall and wide. A host's box stands as
// every receiver's does (`box_tiles`). A decor whose ball's box lies within the reach stands
// on its footprint (`Light::corner_tiles`), and the propagation's way up holds what the decor
// lifts above it (light_propagate.wgsl `up`). The pass runs after decor is placed and the
// world's receivers stood, and before the propagation.

/// The decor compacted this frame as its draws read it (decor_compact.wgsl
/// `decor_compact_cs`): its drawing words, its regions and its instances.
@group(0) @binding(18) var<storage, read> drawing: array<u32>;
@group(0) @binding(19) var<storage, read> regions: array<u32>;
@group(0) @binding(20) var<storage, read> instances: array<u32>;

fn drawing_at(i: u32) -> u32 {
  return select(0u, drawing[i], i < arrayLength(&drawing));
}

fn instance_at(i: u32) -> u32 {
  return select(0u, instances[i], i < arrayLength(&instances));
}

fn instance_cap() -> u32 {
  return arrayLength(&instances) / (DRAWING_COLUMN_CNT + 1u);
}

/// How many slots are placed: the end of the last region, held to the slots there are
/// (decor_compact.wgsl lays the regions).
fn instance_cnt() -> u32 {
  let last = 2u * REGION_CNT - 1u;
  return min(select(0u, regions[last], last < arrayLength(&regions)), instance_cap());
}

/// How far the decor's anchor lies on `axis` from the Q3 position `o`, in units (pxl-content
/// `generation::drawing::ANCHOR`).
fn instance_anchor(axis: u32, o: i32) -> f32 {
  let a = i32(drawing_at(DRAWING_ANCHOR + axis));
  // How far the anchor lies from the eye in whole unit cells, which wrap round the world on X.
  let d = a - (o >> FRAC_W);
  return f32(select(d, wrap_idx(LEAF_CELL_ZOOM, d), axis == 0u)) - f32(o & ((1i << FRAC_W) - 1i)) * (1.0 / f32(1u << FRAC_W));
}

/// Where placed slot `k` stands, as decor_place.wgsl places it: the middle of its base as
/// drawn, in units from the eye at the Q3 position `o`.
fn instance_pos(k: u32, o: vec3<i32>) -> vec3<f32> {
  let cap = instance_cap();
  return vec3<f32>(
    instance_anchor(0u, o.x) + bitcast<f32>(instance_at(k)),
    instance_anchor(1u, o.y) + bitcast<f32>(instance_at(cap + k)),
    instance_anchor(2u, o.z) + bitcast<f32>(instance_at(2u * cap + k)),
  );
}

/// Whether a ball of `radius` at `v` units from the eye meets the eye's view (`eye::sees`). The
/// slopes of the view's sides are the ones the host worked out (`Grid::slope`).
fn sees(v: vec3<f32>, radius: f32) -> bool {
  let f = dot(v, eye_v3(EYE_FORWARD));
  let tan_xy = vec2<f32>(eye_f32(EYE_TAN), eye_f32(EYE_TAN + 1u));
  let o = vec2<f32>(dot(v, eye_v3(EYE_RIGHT)), dot(v, eye_v3(EYE_UP)));
  let slope = vec2<f32>(eye_f32(EYE_GRID + GRID_SLOPE), eye_f32(EYE_GRID + GRID_SLOPE + 1u));
  let side = abs(o) - tan_xy * f <= radius * slope;
  return f >= -radius && side.x && side.y;
}

@compute @workgroup_size(ROW_LANE_CNT, 1, 1)
fn light_mark_draws_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
  var lo: vec3<f32>;
  var hi: vec3<f32>;
  var foot_lo: vec3<f32>;
  var foot_hi: vec3<f32>;
  // A host's box takes a workgroup and shares its tiles among the lanes, as one box may stand
  // over the whole map. Past the host's boxes, a placed decor takes a thread.
  var first = 0u;
  var lanes = 1u;
  if (group.x < LIGHT_RECEIVER_BOX_CAP) {
    if (group.x >= u32(hdr[LIGHT_RECEIVER_BOXES_AT].x)) {
      return;
    }
    lo = bitcast<vec3<f32>>(hdr[LIGHT_RECEIVER_BOXES_AT + 1u + 2u * group.x].xyz);
    hi = bitcast<vec3<f32>>(hdr[LIGHT_RECEIVER_BOXES_AT + 2u + 2u * group.x].xyz);
    foot_lo = lo;
    foot_hi = hi;
    first = lane;
    lanes = ROW_LANE_CNT;
  } else {
    let k = (group.x - LIGHT_RECEIVER_BOX_CAP) * ROW_LANE_CNT + lane;
    if (k >= instance_cnt()) {
      return;
    }
    let o = vec3<i32>(i32(eye_word(EYE_ORIGIN)), i32(eye_word(EYE_ORIGIN + 1u)), i32(eye_word(EYE_ORIGIN + 2u)));
    let base = instance_pos(k, o);
    let middle = decor_middle(base);
    lo = middle - vec3<f32>(decor_radius());
    hi = middle + vec3<f32>(decor_radius());
    let half = vec3<f32>(vec2<f32>(0.5 * f32(WIDEST)), 0.0);
    foot_lo = base - half;
    foot_hi = base + half;
  }
  if (!sees(0.5 * (lo + hi), 0.5 * length(hi - lo))) {
    return;
  }
  if (!light_within(lo, hi, light_reach())) {
    return;
  }
  let b = corner_tiles(foot_lo, foot_hi);
  if (!b.has_tiles) {
    return;
  }
  let across = u32(b.last.x - b.first.x) + 1u;
  let tiles = across * (u32(b.last.y - b.first.y) + 1u);
  for (var t = first; t < tiles; t += lanes) {
    stand(b.first + vec2<i32>(i32(t % across), i32(t / across)));
  }
}
