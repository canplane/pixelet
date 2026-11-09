// pxl-render/frame/shader/location.slot.wgsl

// The beacon reads its draw from the draw list's buffer, in place of a uniform
// of its own (location.wgsl). The buffer is bound once for the pass, and its
// slots lie `DRAW_VEC4S` vec4s apart, as gpu/draw.rs `DrawList::draw_beacon`
// lays them out: the position's Q3 steps, the spin, whether it is night, and
// the position's remainder. A beacon's draw names its slot as its
// first instance (pass/location.rs `draw_beacon`).
@group(1) @binding(0) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

fn draw_at(instance: u32) { draw_slot = instance; }

fn location_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }

fn draw_translation() -> vec3f { return from_eye(location_q(), location_r()); }
