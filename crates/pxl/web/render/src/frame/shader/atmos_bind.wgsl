// The atmosphere's uniform, its transmittance table and the sampler every
// table is read with, at group 0 from binding 8: past the frame's own and
// the chunk's, so one numbering serves every pass that reads the air — the
// tables' own passes, the sky, and each lit pass's shading.

@group(0) @binding(8) var<uniform> atmos: Atmosphere;
@group(0) @binding(9) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(11) var atmos_sampler: sampler;
