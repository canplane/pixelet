// pxl-render/frame/shader/skinned_draw.slot.wgsl

// A character's draw read off the list's uniforms at its slot, which its draw
// names as its first instance (gpu/pass/skinned_mesh.rs `draw`): the list's
// buffer is bound once a pass, a slot `DRAW_VEC4S` words apart.
@group(1) @binding(0) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

// The draw a vertex is of: the one its instance names.
fn draw_at(instance: u32) { draw_slot = instance; }

fn draw_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }
