// pxl-render/mesh/mask.rs

// A chunk box's mask, the one statement of its bits: the mesher sets them
// (`mesh/chunk.rs` `mask`), and build.rs writes them as the WGSL constants
// the shaders read them by. A lateral kind's bits come in fours from
// its base: +X, −X, +Y, −Y. The includer says what `bits!` makes of them.
bits! {
    FACE_BASE = 1 << 0,
    RISES_BASE = 1 << 6,
    DIPS_BASE = 1 << 10,
    DIAG_PP = 1 << 14,
    DIAG_PN = 1 << 15,
    DIAG_NP = 1 << 16,
    DIAG_NN = 1 << 17,
    CREASE_BASE = 1 << 18,
    CAP_PZ = 1 << 22,
    CAP_NZ = 1 << 23,
    DIAG_RISES_PP = 1 << 24,
    DIAG_RISES_PN = 1 << 25,
    DIAG_RISES_NP = 1 << 26,
    DIAG_RISES_NN = 1 << 27,
}
