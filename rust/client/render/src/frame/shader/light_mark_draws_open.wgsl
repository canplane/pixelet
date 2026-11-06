// pxl-render/light_mark_draws_open.wgsl

// The drawn receivers' dispatch (light_mark_draws.wgsl `light_mark_draws_cs`), a buffer of its
// own that this pass writes and that one reads its count from, binding the light's args its
// stands write apart from it.

@group(0) @binding(24) var<storage, read_write> draws: array<u32, 3>;

/// The drawn receivers' workgroups (`light::DRAWS_DISPATCH_WORDS`), the cast's one thread: the
/// host's boxes' threads, and past them the placed decor's where any is placed.
@compute @workgroup_size(1)
fn light_mark_draws_open_cs() {
  let stands = min(u32(head[LIGHT_STANDS_AT].x), LIGHT_STANDS);
  let placed = instance_cnt();
  let room = LIGHT_STANDS + instance_cap();
  let threads = select(stands, LIGHT_STANDS + placed, placed != 0u);
  draws = array<u32, 3>(workgroups(threads, room, ROW_LANES), 1u, 1u);
}
