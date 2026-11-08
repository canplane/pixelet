// pxl-render/frame/shader/skinned_draw.slot.wgsl

// A character's draw reads its uniform from the list's buffer at its slot, which the draw
// names as its first instance (gpu/pass/skinned_mesh.rs `draw`). The list's buffer is bound
// once a pass, with slots `DRAW_VEC4S` vec4 words apart.
@group(1) @binding(0) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

// A vertex belongs to the draw its instance names.
fn draw_at(instance: u32) { draw_slot = instance; }

fn skinned_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }
