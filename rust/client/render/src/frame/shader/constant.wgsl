// Shared constants; luminance weights are Rec. 709, as on the CPU.

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;
const INV_4PI: f32 = 0.0795774715;
const LUMA: vec3f = vec3f(0.2126, 0.7152, 0.0722);
// At true size each disc is eight pixels on a phone, smaller than anyone
// remembers; four is chosen by look. Only size is exaggerated, not light or phase.
const DISC_SCALE: f32 = 4.0;
const MOON_ANGULAR_RADIUS: f32 = 0.004522;
