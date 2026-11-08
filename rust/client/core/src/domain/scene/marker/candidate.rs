// pxl-core/domain/scene/marker/candidate.rs

//! One frame's marker candidates, kept as parallel arrays with one lane per
//! candidate.

#[derive(Clone, Debug, Default)]
pub(super) struct Candidates {
    pub cnt: u32,

    /// The anchor, relative to the origin of the local world frame.
    pub local_x: Vec<f64>,
    pub local_y: Vec<f64>,
    pub local_z: Vec<f64>,

    /// The marker's size, in CSS pixels.
    pub w_px: Vec<f32>,
    pub h_px: Vec<f32>,
    /// The width of the region the marker keeps (`set.rs`), at least its own
    /// width.
    pub region_px: Vec<f32>,
    /// How far below the sprite's top the anchor lies, in CSS pixels.
    pub anchor_px: Vec<f32>,
    /// Used only to keep two candidates at equal distance from swapping
    /// between frames.
    pub tiebreak: Vec<f64>,

    // The selector fills the fields below. They live here so the pass
    // allocates in one place.
    /// The squared distance from the eye. Only its order is used, so no root
    /// is taken.
    pub d2: Vec<f64>,
    /// Its region in viewport pixels: left, top, right, bottom.
    pub screen: Vec<[f64; 4]>,
    /// Indices that passed the frustum.
    pub order: Vec<u32>,
}

impl Candidates {
    /// A high-water mark, not a limit.
    pub(super) fn cap(&self) -> usize {
        self.local_x.len()
    }

    pub(super) fn reset(&mut self) {
        self.cnt = 0;
    }

    pub(super) fn push(
        &mut self,
        local_x: f64,
        local_y: f64,
        local_z: f64,
        [w_px, h_px, region_px]: [f64; 3],
        anchor_px: f64,
        tiebreak: f64,
    ) {
        let i = self.cnt as usize;
        self.cnt += 1;
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
        self.d2.resize(n, 0.0);
        self.screen.resize(n, [0.0; 4]);
        self.order.resize(n, 0);
    }
}
