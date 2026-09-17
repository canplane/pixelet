// A vertex's face mask, the bits the mesher writes (render/layer/pipeline/chunk_mesher.rs): what
// the edge shading asks of a face. The lateral bits come in fours in the
// order +X, -X, +Y, -Y from each base, so a lateral is `base * k` with k
// 1, 2, 4, 8 — chunk.wgsl's lateral_bit. shader.rs's test holds each value
// to the mesher's constant.
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
