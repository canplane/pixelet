// pxl_core/layer/billboard/batch.rs

//! One frame's billboard candidates, as lanes rather than objects.
//!
//! What this replaces was an array of per-candidate objects built fresh every
//! camera tick, handed to a selector that built a second array of its own,
//! which returned a third — precisely the shape deck.gl names as the expensive
//! one ("avoid creating per-item objects in accessors") and the shape Figma
//! removes by keeping objects in preallocated typed arrays.
//!
//! `src` is deliberately not the caller's data but an index back into it: a
//! batch knows where a candidate came from and nothing about what it is, which
//! is what keeps POI, chat bubbles and actors able to share one selector while
//! storing entirely different things.
//!
//! `tiebreak` is a precomputed number, not an id to hash at selection time.
//! The value is stable per candidate for as long as it exists, so hashing it
//! once where the candidate is created costs nothing per frame.

/// A frame's worth of candidates is not a residency set, so it grows to what
/// it meets rather than refusing past a ceiling. Dropping here would be a badge
/// missing from the screen for no reason a viewer could see, and the high-water
/// mark is a property of the densest place visited rather than of how long the
/// session has run. Every lane grows in lockstep, the selector's included, so
/// the whole pass has one capacity.
const INIT_CAP: usize = 1 << 10;

#[derive(Clone, Debug, Default)]
pub struct BillboardBatch {
    pub count: u32,

    /// Position relative to the local world frame's origin (frame/coord.rs).
    pub local_x: Vec<f64>,
    pub local_y: Vec<f64>,
    pub local_z: Vec<f64>,

    /// Full-strength drawn width in CSS pixels, before the distance fade.
    pub w_px: Vec<f32>,
    /// Height that must not be overlapped, which is not the drawn height.
    pub collide_px: Vec<f32>,
    /// Canvas-space Y of the anchor, in the same units as `collide_px`.
    pub anchor_px: Vec<f32>,

    /// Stable per candidate, used only to keep two at equal distance from
    /// swapping places between frames.
    pub tiebreak: Vec<f64>,

    /// The caller's own index for this candidate.
    pub src: Vec<u32>,

    // Filled by the selector rather than the producer. They live here so the
    // whole pass has one capacity and one allocation site.
    /// World position, resolved once from the local one.
    pub world_x: Vec<f64>,
    pub world_y: Vec<f64>,
    pub world_z: Vec<f64>,
    /// Squared distance to the eye. Squared because only the order is used.
    pub d2: Vec<f64>,
    /// Candidate indices that passed the frustum, in the order they will be
    /// offered to the placement pass.
    pub order: Vec<u32>,
    /// How many candidates were folded into this one because they wanted the
    /// pixels it took. Meaningful only for the ones the selector kept, and
    /// zero there means it stands alone.
    pub absorbed: Vec<u32>,
}

impl BillboardBatch {
    pub fn new() -> BillboardBatch {
        let mut b = BillboardBatch::default();
        b.reserve(INIT_CAP);
        b
    }

    /// Backing capacity, which is the high-water mark rather than a limit.
    pub fn cap(&self) -> usize {
        self.src.len()
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push(&mut self, src: u32, local_x: f64, local_y: f64, local_z: f64, w_px: f64, collide_px: f64, anchor_px: f64, tiebreak: f64) {
        let i = self.count as usize;
        self.count += 1;
        if i >= self.cap() {
            self.reserve((i + 1).max(self.cap() * 2));
        }
        self.local_x[i] = local_x;
        self.local_y[i] = local_y;
        self.local_z[i] = local_z;
        self.w_px[i] = w_px as f32;
        self.collide_px[i] = collide_px as f32;
        self.anchor_px[i] = anchor_px as f32;
        self.tiebreak[i] = tiebreak;
        self.src[i] = src;
    }

    fn reserve(&mut self, n: usize) {
        self.local_x.resize(n, 0.0);
        self.local_y.resize(n, 0.0);
        self.local_z.resize(n, 0.0);
        self.w_px.resize(n, 0.0);
        self.collide_px.resize(n, 0.0);
        self.anchor_px.resize(n, 0.0);
        self.tiebreak.resize(n, 0.0);
        self.src.resize(n, 0);
        self.world_x.resize(n, 0.0);
        self.world_y.resize(n, 0.0);
        self.world_z.resize(n, 0.0);
        self.d2.resize(n, 0.0);
        self.order.resize(n, 0);
        self.absorbed.resize(n, 0);
    }
}

/// FNV-1a into [0,1), over the key's UTF-16 units as `charCodeAt` reads them.
/// The same shape as the chunks' stable hash the scheduler blends by, used
/// here once per candidate's lifetime rather than once per frame.
pub fn stable_hash(key: &str) -> f64 {
    let mut hash: u32 = 2_166_136_261;
    for unit in key.encode_utf16() {
        hash ^= unit as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash as f64 / 4_294_967_296.0
}
