// pxl-render/light_mark_draws.wgsl

// The drawn receivers of the shadow map, each the eye sees standing as the world's do
// (light_mark.wgsl): the actors the host draws, a thread each (`light::ACTORS_AT`); then
// the decor generated, a thread a slot of its instances (gpu/generator.rs), as many as
// are placed, where each stands, as decor_compact.wgsl lays it. Seen
// where a ball about its middle as wide as decor is tall and wide meets the view
// (`eye::sees`); the tiles about its own stood on too, as many as half the widest decor
// spans (`Light::ring`). The actors are those the scene draws in the world
// (gpu/ray/cast.rs `Cast::actors`), no more than `light::ACTORS`. Run after decor is
// placed, before the propagation.

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
/// (`Instances::cnt`).
fn instance_cnt() -> u32 {
  let last = 2u * REGIONS - 1u;
  return min(select(0u, regions[last], last < arrayLength(&regions)), instance_cap());
}

/// The anchor on `axis` from a Q3 `o`, in units (`Instances::anchor`).
fn instance_anchor(axis: u32, o: i32) -> f32 {
  let a = i32(drawing_at(DRAWN_ANCHOR + axis));
  // The anchor and the eye in unit cells, which wrap round the world on X.
  let d = a - (o >> FRAC_W);
  return f32(select(d, wrap_idx(LEAF_CELL, d), axis == 0u)) - f32(o & ((1i << FRAC_W) - 1i)) / f32(1u << FRAC_W);
}

/// Where placed slot `k` stands, its base middle as drawn, units from the eye at Q3
/// `o` (`Instances::pos`).
fn instance_pos(k: u32, o: vec3<i32>) -> vec3<f32> {
  let cap = instance_cap();
  return vec3<f32>(
    instance_anchor(0u, o.x) + bitcast<f32>(instance_at(k)),
    instance_anchor(1u, o.y) + bitcast<f32>(instance_at(cap + k)),
    instance_anchor(2u, o.z) + bitcast<f32>(instance_at(2u * cap + k)),
  );
}

/// Whether a ball of `radius` at `v` units from the eye meets its view (`eye::sees`).
fn sees(v: vec3<f32>, radius: f32) -> bool {
  let f = dot(v, eye_v3(FORWARD));
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let o = vec2<f32>(dot(v, eye_v3(RIGHT)), dot(v, eye_v3(UP)));
  let side = abs(o) - tan_xy * f <= radius * sqrt(vec2<f32>(1.0) + tan_xy * tan_xy);
  return f >= -radius && side.x && side.y;
}

/// How many tiles either side of its own a drawn receiver stands on (`Light::ring`).
fn ring() -> i32 {
  let across = light_v3(LIGHT_ACROSS);
  let tile = f32(LIGHT_TILE) * sqrt(dot(across, across));
  return max(i32(ceil(f32(WIDEST) / 2.0 / tile)), 1i);
}

@compute @workgroup_size(ROW_LANES, 1, 1)
fn light_mark_draws_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  var v: vec3<f32>;
  if (id.x < LIGHT_ACTORS) {
    if (id.x >= u32(head[LIGHT_ACTORS_AT].x)) {
      return;
    }
    v = bitcast<vec3<f32>>(head[LIGHT_ACTORS_AT + 1u + id.x].xyz);
  } else {
    let k = id.x - LIGHT_ACTORS;
    if (k >= instance_cnt()) {
      return;
    }
    let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
    v = instance_pos(k, o);
  }
  let middle = v + vec3<f32>(0.0, 0.0, f32(TALLEST) / 2.0);
  if (!sees(middle, f32(TALLEST + WIDEST))) {
    return;
  }
  let t = tile_of(v);
  let current = cast_number();
  let r = ring();
  for (var b = -r; b <= r; b += 1i) {
    for (var a = -r; a <= r; a += 1i) {
      stand(current, t + vec2<i32>(a, b));
    }
  }
}
