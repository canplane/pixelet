// pxl-kernel/billboard/batch.rs

//! One frame's billboard candidates, as lanes.

#[derive(Clone, Debug, Default)]
pub struct BillboardBatch {
    pub count: u32,

    /// Relative to the local world frame's origin.
    pub local_x: Vec<f64>,
    pub local_y: Vec<f64>,
    pub local_z: Vec<f64>,

    /// CSS pixels.
    pub w_px: Vec<f32>,
    pub h_px: Vec<f32>,
    /// The width of the region it keeps (`field.rs`), at least its own.
    pub region_px: Vec<f32>,
    /// Canvas-space Y of the anchor, CSS pixels.
    pub anchor_px: Vec<f32>,
    /// Only keeps two at equal distance from swapping between frames.
    pub tiebreak: Vec<f64>,

    // Filled by the selector, kept here so the pass has one allocation site.
    /// Laid on the planet, in the root frame.
    pub world_x: Vec<f64>,
    pub world_y: Vec<f64>,
    pub world_z: Vec<f64>,
    /// Squared: only the order is used.
    pub d2: Vec<f64>,
    /// Its region in viewport pixels: left, top, right, bottom.
    pub screen: Vec<[f64; 4]>,
    /// Indices that passed the frustum.
    pub order: Vec<u32>,
}

impl BillboardBatch {
    /// A high-water mark, not a limit.
    pub fn cap(&self) -> usize {
        self.local_x.len()
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push(&mut self, local_x: f64, local_y: f64, local_z: f64, [w_px, h_px, region_px]: [f64; 3], anchor_px: f64, tiebreak: f64) {
        let i = self.count as usize;
        self.count += 1;
        if i >= self.cap() {
            self.reserve((i + 1).max(self.cap() * 2));
        }
        self.local_x[i] = local_x;
        self.local_y[i] = local_y;
        self.local_z[i] = local_z;
        self.w_px[i] = w_px as f32;
        self.h_px[i] = h_px as f32;
        self.region_px[i] = region_px.max(w_px) as f32;
        self.anchor_px[i] = anchor_px as f32;
        self.tiebreak[i] = tiebreak;
    }

    fn reserve(&mut self, n: usize) {
        self.local_x.resize(n, 0.0);
        self.local_y.resize(n, 0.0);
        self.local_z.resize(n, 0.0);
        self.w_px.resize(n, 0.0);
        self.h_px.resize(n, 0.0);
        self.region_px.resize(n, 0.0);
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
