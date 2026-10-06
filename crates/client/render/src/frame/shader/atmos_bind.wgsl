// Bindings from 8 on, after the frame's and the chunk's, so every pass that
// reads the air shares one numbering; the words at 8 are atmos_medium.wgsl's.

@group(0) @binding(9) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(11) var atmos_sampler: sampler;
