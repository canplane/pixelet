// The air's bindings start at 8, after the lit passes' own 0 to 7, so every
// pass that reads the air numbers them alike. The words at binding 8 are
// declared in atmos_medium.wgsl.

@group(0) @binding(9) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(11) var atmos_sampler: sampler;
