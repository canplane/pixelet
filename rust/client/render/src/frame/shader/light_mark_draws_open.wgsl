// pxl-render/light_mark_draws_open.wgsl

// Writes the drawn receivers' dispatch (light_mark_draws.wgsl `light_mark_draws_cs`). The
// dispatch is a buffer of its own, which this pass writes and that pass reads its workgroups
// from. That pass writes the light's args as it stands tiles, and a dispatch cannot read its
// workgroups from a buffer it writes.

@group(0) @binding(24) var<storage, read_write> draws: array<u32, 3>;

/// Writes the drawn receivers' workgroups (`light::DRAWS_DISPATCH_WORDS`), one thread for the
/// cast. There is one workgroup for each box a host's draw stands within, and, where any decor
/// is placed, the placed decor's workgroups past `LIGHT_RECEIVER_BOX_CAP`.
@compute @workgroup_size(1)
fn light_mark_draws_open_cs() {
  let stands = min(u32(hdr[LIGHT_RECEIVER_BOXES_AT].x), LIGHT_RECEIVER_BOX_CAP);
  let placed = instance_cnt();
  let groups = select(stands, LIGHT_RECEIVER_BOX_CAP + workgroup_cnt(placed, instance_cap(), ROW_LANE_CNT), placed != 0u);
  draws = array<u32, 3>(min(groups, WORKGROUP_CAP), 1u, 1u);
}
