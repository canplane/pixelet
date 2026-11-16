// pxl-render/frame/shader/vertex.wgsl

// Where a corner of a draw that reads no vertex buffer lies, worked out from its number's bits:
// a phone's driver refuses a table of vectors read at a place found as the shader runs.

/// Corner `i` of the triangle that covers the screen, in clip space: (-1, -1), (3, -1) and
/// (-1, 3).
fn screen_triangle_corner(i: u32) -> vec2f {
  return vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
}

/// Corner `k` of a quad drawn as two triangles, (0, 0), (1, 0), (1, 1) and (0, 0), (1, 1),
/// (0, 1), as its steps across and down.
fn quad_corner(k: u32) -> vec2u {
  return vec2u(u32(k == 1u || k == 2u || k == 4u), u32(k == 2u || k == 4u || k == 5u));
}

/// Corner `k` of a quad drawn as a strip of four, (0, 0), (1, 0), (0, 1) and (1, 1), as its
/// steps across and down.
fn strip_corner(k: u32) -> vec2u {
  return vec2u(k & 1u, k >> 1u);
}
