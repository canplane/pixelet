// The cloud layer's shape, baked once into a tile (gpu/clouds.rs): the
// field is the same everywhere on the map, so it is drawn into a texture at
// boot and read back with a texture fetch a pixel rather than the eight
// octaves of hashes it takes to make. The tile is a square of the clouds'
// own lattice, and the lattice wraps with it, so the pattern meets itself:
// 32 lattice units is some 250 km, further than the layer is ever drawn.
const CLOUD_TILE_UNITS: u32 = 32u;
const CLOUD_TILE_TEXELS: u32 = 1024u;
