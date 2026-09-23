// The numbers every shader shares, put ahead of each one's own files
// (record.rs's wgsl!): pi and what is made of it, and the luminance of a
// linear colour (Rec. 709), the weights look/environment.rs's luminance
// takes on the CPU.

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;
const INV_4PI: f32 = 0.0795774715;
const LUMA: vec3f = vec3f(0.2126, 0.7152, 0.0722);
