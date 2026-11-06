// pxl-render/light_claim.wgsl

// The tiles the propagation wanted claimed nearest first (light_propagate.wgsl), dispatched
// off the claim's own dispatch (`light::CLAIM_DISPATCH`), which this pass does not bind: it
// writes the light's args, the places, the lists and their rows, apart from it.

/// Wanted tile `t` claimed by the cast numbered `current` at place `place`: its word the cast's,
/// owed no more; its place laid for its slots' parked rays, each slot's state taken from the
/// cast before's region where that cast claimed it and left it owed and the light carries
/// (`light::READ_OWED`, `light::HEAD_CARRIES`), so a ray its budget did not finish goes on and
/// a slot that cast answered keeps its word (`ray::KEPT`), else set to no parked ray, so a
/// slot's ray starts afresh; and what feeds it listed (`enlist`).
fn claim(t: vec2<i32>, place: u32, current: u32) {
  let n = light_tiles();
  let w = reads_at() + u32(t.y) * n + u32(t.x);
  let word = list_at(w);
  let held = head_word(LIGHT_HEAD_CARRIES) != 0u && (word & LIGHT_READ_OWED) != 0u
    && (word & ~LIGHT_READ_OWED) + 1u == current;
  let at = light_place_at(texel_slot(vec2<u32>(t) * LIGHT_TILE));
  let was = list_at(at);
  list_put(at, place);
  for (var k = 0u; k < LIGHT_TILE * LIGHT_TILE; k += 1u) {
    let dst = light_state_at(place, k, current);
    if (held) {
      let src = light_state_at(was, k, current - 1u);
      for (var i = 0u; i < STATE_WORDS; i += 1u) {
        list_put(dst + i, list_at(src + i));
      }
      let last = dst + STATE_WORDS - 1u;
      if ((list_at(last) & PARKED) == 0u) {
        list_put(last, KEPT);
      }
    } else {
      list_put(dst + STATE_WORDS - 1u, 0u);
    }
  }
  list_put(w, current);
  enlist(current, t);
}

/// The wanted tiles claimed nearest first, a thread a tile wanted (`light::claims`): every one
/// of a ring that fits within `light::CLAIM_BUDGET` with the rings within it, and of the one
/// ring that straddles it the first to take a place, none past; each a place of its own among
/// the cast's claims (`light::ARGS_PLACED`). A tile not claimed stays wanted, its word as it
/// was, is counted as the cast's owed work (`light::ARGS_LEFT`, `node_cache::OWED_AT`), and is
/// wanted again by the next cast that reads it. Run after the propagation.
@compute @workgroup_size(LIGHT_LISTED_LANES, 1, 1)
fn light_claim_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let e = (group.y * LIGHT_LIST_ACROSS + group.x) * LIGHT_LISTED_LANES + lane.x;
  let n = light_tiles();
  if (e >= in_room(atomicLoad(&args.counts[LIGHT_LISTED_WANTED]), n * n)) {
    return;
  }
  let w = list_at(listed_at(LIGHT_LISTED_WANTED) + 1u + e);
  let t = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let rings = head_word(LIGHT_HEAD_RINGS);
  let r = ring(t);
  var before = 0u;
  for (var q = 0u; q < r; q += 1u) {
    before += atomicLoad(&args.marks[rings + q]);
  }
  let own = atomicLoad(&args.marks[rings + r]);
  var claimed = before + own <= LIGHT_CLAIM_BUDGET;
  if (!claimed && before < LIGHT_CLAIM_BUDGET) {
    claimed = atomicAdd(&args.passes[LIGHT_ARGS_STRADDLE], 1u) < LIGHT_CLAIM_BUDGET - before;
  }
  if (!claimed) {
    atomicAdd(&args.passes[LIGHT_ARGS_LEFT], 1u);
    return;
  }
  let place = atomicAdd(&args.passes[LIGHT_ARGS_PLACED], 1u);
  if (place < LIGHT_CLAIM_BUDGET) {
    claim(t, place, cast_number());
  }
}
