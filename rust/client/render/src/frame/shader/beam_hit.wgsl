// pxl-render/beam_hit.wgsl

// The beam rays' walk over the node cache alone (walk.wgsl), either observer's (beam.eye.wgsl,
// beam.light.wgsl), its line kept as it goes, the node it ended or was left at among it. One it
// leaves is listed for `beam_cs` (beam.wgsl), which goes on from there writing what it asks; one
// that ends is left as that leaves it.

/// `beam_cs`'s dispatch, then how many are listed (list.wgsl `Left`): only the
/// list's own words.
@group(0) @binding(10) var<storage, read_write> args: Left;

@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn beam_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let c = beam_item(id.xy);
  let lattice = beam_lattice();
  if (c.x >= lattice.x || c.y >= lattice.y) {
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
  let hand = beam_hand_at(c);
  if (w.ends == 4u) {
    // The list holds a place for each beam ray of the lattice, each listed once.
    let place = enlist(lattice.x * lattice.y);
    if (place < lattice.x * lattice.y) {
      beams[corners_left_at() + 1u + place] = c.x | (c.y << 16u);
    }
    // How far it came, where `beam_cs` goes on from its line's node (resume.wgsl
    // `line_resumed`), and writes this word as below.
    beams[hand] = bitcast<u32>(w.t);
    return;
  }
  beams[hand] = bitcast<u32>(beam_hand(w.t));
}
