// pxl-render/frame/shader/location.slot.wgsl

// The beacon's draw read off the list's uniforms at its slot, which its draw
// names as its first instance (gpu/pass/location.rs `draw_beacon`), in place
// of its own uniform (location.wgsl): the list's buffer bound once a pass, a
// slot `DRAW_VEC4S` words apart, as frame/draw.rs `DrawList::draw_beacon`
// lays it: its position's Q3, its spin, its opacity, whether it is night,
// and its position's remainder.
@group(1) @binding(0) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

// The draw a vertex is of: the one its instance names.
fn draw_at(instance: u32) { draw_slot = instance; }

fn draw_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }

fn draw_translation() -> vec3f { return from_eye(bitcast<vec3i>(draw_word(0u).xyz), draw_word(2u).xyz); }
fn draw_fourth() -> f32 { return draw_word(0u).w; }
fn draw_night() -> f32 { return draw_word(1u).y; }
