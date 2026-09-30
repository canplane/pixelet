// pxl-kernel/layer/billboard/batch.rs

//! One frame's billboard candidates, as lanes rather than objects.
//!
//! Candidate membership belongs to the caller. These lanes contain only
//! presentation inputs and the screen selector's scratch values.

#[derive(Clone, Debug, Default)]
pub struct BillboardBatch {
    pub count: u32,

    /// Position relative to the local world frame's origin (frame/coord.rs).
    pub local_x: Vec<f64>,
    pub local_y: Vec<f64>,
    pub local_z: Vec<f64>,

    /// Full-strength drawn width in CSS pixels, before the distance fade.
    pub w_px: Vec<f32>,
    /// Canvas-space Y of the anchor, in CSS pixels before the fade.
    pub anchor_px: Vec<f32>,
    /// Stable per candidate, used only to keep two at equal distance from
    /// swapping places between frames.
    pub tiebreak: Vec<f64>,

    // Filled by the selector rather than the producer. They live here so the
    // whole pass has one capacity and one allocation site.
    /// The position laid on the planet, in the root frame, resolved once
    /// from the local one.
    pub world_x: Vec<f64>,
    pub world_y: Vec<f64>,
    pub world_z: Vec<f64>,
    /// Squared distance to the eye. Squared because only the order is used.
    pub d2: Vec<f64>,
    /// Where a candidate kept is on the screen, in viewport pixels: its box
    /// as drawn, left, top, right, bottom.
    pub screen: Vec<[f64; 4]>,
    /// Candidate indices that passed the frustum, in the order they will be
    /// offered to the placement pass.
    pub order: Vec<u32>,
}

impl BillboardBatch {
    /// Backing capacity, which is the high-water mark rather than a limit.
    pub fn cap(&self) -> usize {
        self.local_x.len()
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    pub fn push(&mut self, local_x: f64, local_y: f64, local_z: f64, w_px: f64, anchor_px: f64, tiebreak: f64) {
        let i = self.count as usize;
        self.count += 1;
        if i >= self.cap() {
            self.reserve((i + 1).max(self.cap() * 2));
        }
        self.local_x[i] = local_x;
        self.local_y[i] = local_y;
        self.local_z[i] = local_z;
        self.w_px[i] = w_px as f32;
        self.anchor_px[i] = anchor_px as f32;
        self.tiebreak[i] = tiebreak;
    }

    fn reserve(&mut self, n: usize) {
        self.local_x.resize(n, 0.0);
        self.local_y.resize(n, 0.0);
        self.local_z.resize(n, 0.0);
        self.w_px.resize(n, 0.0);
        self.anchor_px.resize(n, 0.0);
        self.tiebreak.resize(n, 0.0);
        self.world_x.resize(n, 0.0);
        self.world_y.resize(n, 0.0);
        self.world_z.resize(n, 0.0);
        self.d2.resize(n, 0.0);
        self.screen.resize(n, [0.0; 4]);
        self.order.resize(n, 0);
    }
}
