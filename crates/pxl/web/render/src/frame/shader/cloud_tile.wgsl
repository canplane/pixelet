// 32 lattice units is about 250 km, further than the layer is drawn, and the
// lattice wraps with the tile.
const CLOUD_TILE_UNITS: u32 = 32u;
const CLOUD_TILE_TEXELS: u32 = 1024u;
// Optical depth of a full cloud: past a few its underside is self-shadowed.
const CLOUD_DEPTH: f32 = 6.0;
// The logistic's scale that approximates the normal CDF to within 0.01, which
// takes a near-normal noise to an even share.
const LOGISTIC_SCALE: f32 = 1.702;
