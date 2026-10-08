// The mesher's face mask bits; lateral bits come in fours (+X, -X, +Y, -Y)
// from each base, as the mesher's constants.
const FACE_BASE: u32 = 1u;
const RISES_BASE: u32 = 64u;
const DIPS_BASE: u32 = 1024u;
const DIAG_PP: u32 = 16384u;
const DIAG_PN: u32 = 32768u;
const DIAG_NP: u32 = 65536u;
const DIAG_NN: u32 = 131072u;
const CREASE_BASE: u32 = 262144u;
const CAP_PZ: u32 = 4194304u;
const CAP_NZ: u32 = 8388608u;
const DIAG_RISES_PP: u32 = 16777216u;
const DIAG_RISES_PN: u32 = 33554432u;
const DIAG_RISES_NP: u32 = 67108864u;
const DIAG_RISES_NN: u32 = 134217728u;
const UNDER_AS_TOP: u32 = 268435456u;
