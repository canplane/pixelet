// pxl-render/light_mark_draws_open.wgsl

// The drawn receivers' dispatch (light_mark_draws.wgsl `light_mark_draws_cs`), a buffer of its
// own that this pass writes and that one reads its count from, binding the light's args its
// stands write apart from it.

@group(0) @binding(24) var<storage, read_write> draws: array<u32, 3>;

/// The drawn receivers' workgroups (`light::DRAWS_DISPATCH_WORDS`), the cast's one thread: one
/// a host's box, and past them the placed decor's where any is placed.
@compute @workgroup_size(1)
fn light_mark_draws_open_cs() {
  let stands = min(u32(header[LIGHT_RECEIVER_BOXES_AT].x), LIGHT_RECEIVER_BOXES);
  let placed = instance_cnt();
  let groups = select(stands, LIGHT_RECEIVER_BOXES + workgroups(placed, instance_cap(), ROW_LANES), placed != 0u);
  draws = array<u32, 3>(min(groups, WORKGROUPS_CAP), 1u, 1u);
}
