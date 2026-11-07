// pxl-render/light_mark_draws.wgsl

// The drawn receivers of the shadow map, each the eye sees standing by the world's one rule
// (light_mark.wgsl, `Light::box_tiles`): what the host draws that reads the map, the box each
// stands within as its draw is recorded, a workgroup each (`light::STANDS_AT`); then
// the decor generated, a thread a slot of its instances (gpu/generator.rs), as many as
// are placed, where each stands, as decor_compact.wgsl lays it, the workgroups as many as the
// cast's one thread opens in a buffer of their own, which this pass reads its count from and
// does not bind (light_mark_draws_open.wgsl, `light::DRAWS_DISPATCH_WORDS`). Seen where a
// ball about its box, a placed decor's about its middle as wide as decor is tall and wide,
// meets the view (`eye::sees`); a host's box stands, as every receiver's does (`box_tiles`), and
// a decor within the reach by that box stands on its footprint (`Light::corner_tiles`), the
// propagation's way up holding what it lifts over it (light_propagate.wgsl `up`). The host's
// are those gpu/draw.rs `DrawList` records, no more than `light::STANDS`. Run after decor is
// placed and the world's receivers stood, before the propagation.

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

/// The slots the instances hold.
fn instance_cap() -> u32 {
  return arrayLength(&instances) / (DRAWN_COLUMNS + 1u);
}

/// How many are placed: the last region's end, no more than the slots
/// (decor_compact.wgsl lays the regions).
fn instance_cnt() -> u32 {
  let last = 2u * REGIONS - 1u;
  return min(select(0u, regions[last], last < arrayLength(&regions)), instance_cap());
}

/// The anchor on `axis` from a Q3 `o`, in units (pxl-content `generation::drawn::ANCHOR`).
fn instance_anchor(axis: u32, o: i32) -> f32 {
  let a = i32(drawing_at(DRAWN_ANCHOR + axis));
  // The anchor and the eye in unit cells, which wrap round the world on X.
  let d = a - (o >> FRAC_W);
  return f32(select(d, wrap_idx(LEAF_CELL, d), axis == 0u)) - f32(o & ((1i << FRAC_W) - 1i)) * (1.0 / f32(1u << FRAC_W));
}

/// Where placed slot `k` stands, its base middle as drawn, units from the eye at Q3
/// `o`, as decor_place.wgsl places it.
fn instance_pos(k: u32, o: vec3<i32>) -> vec3<f32> {
  let cap = instance_cap();
  return vec3<f32>(
    instance_anchor(0u, o.x) + bitcast<f32>(instance_at(k)),
    instance_anchor(1u, o.y) + bitcast<f32>(instance_at(cap + k)),
    instance_anchor(2u, o.z) + bitcast<f32>(instance_at(2u * cap + k)),
  );
}

/// Whether a ball of `radius` at `v` units from the eye meets its view (`eye::sees`), its
/// sides' slopes the host's (`Grid::slope`).
fn sees(v: vec3<f32>, radius: f32) -> bool {
  let f = dot(v, eye_v3(FORWARD));
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let o = vec2<f32>(dot(v, eye_v3(RIGHT)), dot(v, eye_v3(UP)));
  let slope = vec2<f32>(eye_f32(GRID + GRID_SLOPE), eye_f32(GRID + GRID_SLOPE + 1u));
  let side = abs(o) - tan_xy * f <= radius * slope;
  return f >= -radius && side.x && side.y;
}

@compute @workgroup_size(ROW_LANES, 1, 1)
fn light_mark_draws_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
  var lo: vec3<f32>;
  var hi: vec3<f32>;
  var foot_lo: vec3<f32>;
  var foot_hi: vec3<f32>;
  // A host's box a workgroup, its tiles among the lanes, as one may stand over the whole map;
  // past them a placed decor a thread.
  var first = 0u;
  var lanes = 1u;
  if (group.x < LIGHT_STANDS) {
    if (group.x >= u32(head[LIGHT_STANDS_AT].x)) {
      return;
    }
    lo = bitcast<vec3<f32>>(head[LIGHT_STANDS_AT + 1u + 2u * group.x].xyz);
    hi = bitcast<vec3<f32>>(head[LIGHT_STANDS_AT + 2u + 2u * group.x].xyz);
    foot_lo = lo;
    foot_hi = hi;
    first = lane;
    lanes = ROW_LANES;
  } else {
    let k = (group.x - LIGHT_STANDS) * ROW_LANES + lane;
    if (k >= instance_cnt()) {
      return;
    }
    let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
    let base = instance_pos(k, o);
    let middle = decor_middle(base);
    lo = middle - vec3<f32>(decor_radius());
    hi = middle + vec3<f32>(decor_radius());
    let half = vec3<f32>(vec2<f32>(0.5 * f32(WIDEST)), 0.0);
    foot_lo = base - half;
    foot_hi = base + half;
  }
  // Seen where the ball about the box meets the view.
  if (!sees(0.5 * (lo + hi), 0.5 * length(hi - lo))) {
    return;
  }
  if (!light_within(lo, hi, light_reach())) {
    return;
  }
  let b = corner_tiles(foot_lo, foot_hi);
  if (!b.got) {
    return;
  }
  let across = u32(b.last.x - b.first.x) + 1u;
  let tiles = across * (u32(b.last.y - b.first.y) + 1u);
  for (var t = first; t < tiles; t += lanes) {
    stand(b.first + vec2<i32>(i32(t % across), i32(t / across)));
  }
}
