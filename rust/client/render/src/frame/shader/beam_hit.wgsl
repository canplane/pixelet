// pxl-render/beam_hit.wgsl

// The beam rays' walk over the node cache alone (walk.wgsl), either observer's (beam_eye.wgsl,
// beam_light.wgsl), its line kept as it goes, the node it ended or was left at among it. One it
// leaves is listed for `beam_cs` (beam.wgsl), which goes on from there writing what it asks; one
// that ends is left as that leaves it.

/// `beam_cs`'s dispatch, then how many are listed: only the list's own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn beam_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let c = beam_item(id.xy);
  let lattice = beam_lattice();
  if (c.x >= lattice.x || c.y >= lattice.y || !beam_needed(c)) {
    return;
  }
  let base = beam_line(c);
  let aim = beam_aim(c);
  beam_lay_origin(base, aim.origin);
  let w = walk(aim, root_start(), base, beam_spacing());
  let at = base + LINE_NODE;
  beams[at + NODE_ZOOM] = w.zoom;
  beams[at + NODE_CORNER] = w.corner.x;
  beams[at + NODE_CORNER + 1u] = w.corner.y;
  beams[at + NODE_CORNER + 2u] = w.corner.z;
  beams[at + NODE_ROOTS] = u32(w.roots.x);
  beams[at + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[at + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[base + LINE_VISITS] = w.visits;
  let hand = beam_hand_at(c);
  if (w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % (CORNER_SIDE * CORNER_SIDE) == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[corners_left_at() + 1u + place] = c.x | (c.y << 16u);
    // How far it came, where `beam_cs` goes on from its line's node (resume.wgsl
    // `line_resumed`), and writes this word as below.
    beams[hand] = bitcast<u32>(w.t);
    return;
  }
  beams[hand] = bitcast<u32>(beam_hand(w.t));
}
