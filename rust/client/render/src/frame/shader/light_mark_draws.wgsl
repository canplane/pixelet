// pxl-render/light_mark_draws.wgsl

// The drawn receivers of the shadow map, each the eye sees standing by the world's one rule
// (light_mark.wgsl, `Light::box_tiles`): what the host draws that reads the map, the box each
// stands within as its draw is recorded, a thread each (`light::STANDS_AT`); then
// the decor generated, a thread a slot of its instances (gpu/generator.rs), as many as
// are placed, where each stands, as decor_compact.wgsl lays it, the threads as many as the
// cast's one thread opens (`light_mark_draws_open_cs`, `light::ARGS_DRAWS`). Seen where a
// ball about its box, a placed decor's about its middle as wide as decor is tall and wide,
// meets the view (`eye::sees`); the box stands, as every receiver's does (`box_tiles`). The
// host's are those frame/draw.rs `DrawList` records, no more than `light::STANDS`. Run after
// decor is placed and the world's receivers stood, before the propagation.

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
fn light_mark_draws_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  var lo: vec3<f32>;
  var hi: vec3<f32>;
  if (id.x < LIGHT_STANDS) {
    if (id.x >= u32(head[LIGHT_STANDS_AT].x)) {
      return;
    }
    lo = bitcast<vec3<f32>>(head[LIGHT_STANDS_AT + 1u + 2u * id.x].xyz);
    hi = bitcast<vec3<f32>>(head[LIGHT_STANDS_AT + 2u + 2u * id.x].xyz);
  } else {
    let k = id.x - LIGHT_STANDS;
    if (k >= instance_cnt()) {
      return;
    }
    let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
    let middle = decor_middle(instance_pos(k, o));
    lo = middle - vec3<f32>(decor_radius());
    hi = middle + vec3<f32>(decor_radius());
  }
  // Seen where the ball about the box meets the view.
  if (!sees(0.5 * (lo + hi), 0.5 * length(hi - lo))) {
    return;
  }
  let b = box_tiles(lo, hi);
  if (!b.got) {
    return;
  }
  for (var j = b.first.y; j <= b.last.y; j += 1i) {
    for (var i = b.first.x; i <= b.last.x; i += 1i) {
      stand_drawn(vec2<i32>(i, j));
    }
  }
}

/// The drawn receivers' workgroups (`light::ARGS_DRAWS`), the cast's one thread: the host's
/// boxes' threads, and past them the placed decor's where any is placed.
@compute @workgroup_size(1)
fn light_mark_draws_open_cs() {
  let stands = min(u32(head[LIGHT_STANDS_AT].x), LIGHT_STANDS);
  let placed = instance_cnt();
  let room = LIGHT_STANDS + instance_cap();
  let threads = select(stands, LIGHT_STANDS + placed, placed != 0u);
  atomicStore(&light_args[LIGHT_ARGS_DRAWS], workgroups(threads, room, ROW_LANES));
  atomicStore(&light_args[LIGHT_ARGS_DRAWS + 1u], 1u);
  atomicStore(&light_args[LIGHT_ARGS_DRAWS + 2u], 1u);
}
