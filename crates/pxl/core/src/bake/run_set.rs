//! A chunk's column runs.
//!
//! One block of lanes in carve order, widest element first, so a set read
//! back out of the block it was written into needs no translation and the
//! views below are the block itself.

/// Views over one run set's lanes.
pub struct RunSet<'a> {
    pub count: u32,
    /// Columns on a side; column `(cx, cy)` is `cy * span + cx`.
    pub span: u32,
    pub start: &'a [u32],
    pub z0: &'a [i32],
    pub z1: &'a [i32],
    pub layer_taps: &'a [u32],
    pub r: &'a [u8],
    pub g: &'a [u8],
    pub b: &'a [u8],
    pub material_class: &'a [u8],
    pub sub_ox: &'a [u8],
    pub sub_oy: &'a [u8],
    pub sub_oz: &'a [u8],
    pub sub_sx: &'a [u8],
    pub sub_sy: &'a [u8],
    pub sub_sz: &'a [u8],
}

const RUN_I32_FIELDS: u32 = 3;
const RUN_U8_FIELDS: u32 = 10;

/// The bytes a set of `count` runs over `span`² columns occupies.
pub fn run_set_bytes(count: u32, span: u32) -> u32 {
    (span * span + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS)
}

impl<'a> RunSet<'a> {
    /// Carves the lanes out of `bytes`, which must start 4-aligned and hold
    /// at least `run_set_bytes(count, span)`; both are checked, so a wrong
    /// block is a panic and never a misread.
    pub fn carve(bytes: &'a [u8], count: u32, span: u32) -> RunSet<'a> {
        let need = run_set_bytes(count, span) as usize;
        assert!(bytes.len() >= need, "run set of {} bytes shorter than {}", bytes.len(), need);
        assert!((bytes.as_ptr() as usize).is_multiple_of(4), "run set not 4-aligned");
        let base = bytes.as_ptr();
        let n = count as usize;
        let cols = (span * span + 1) as usize;
        let mut at = 0usize;
        // SAFETY: every span below lies inside `bytes` — the length was
        // checked against the layout's total — and the word lanes start
        // 4-aligned, as the base was checked to.
        let mut words = |len: usize| unsafe {
            let p = base.add(at);
            at += len * 4;
            p
        };
        let start = unsafe { core::slice::from_raw_parts(words(cols) as *const u32, cols) };
        let z0 = unsafe { core::slice::from_raw_parts(words(n) as *const i32, n) };
        let z1 = unsafe { core::slice::from_raw_parts(words(n) as *const i32, n) };
        let layer_taps = unsafe { core::slice::from_raw_parts(words(n) as *const u32, n) };
        let mut bytes_at = at;
        let mut lane = || unsafe {
            let s = core::slice::from_raw_parts(base.add(bytes_at), n);
            bytes_at += n;
            s
        };
        RunSet {
            count,
            span,
            start,
            z0,
            z1,
            layer_taps,
            r: lane(),
            g: lane(),
            b: lane(),
            material_class: lane(),
            sub_ox: lane(),
            sub_oy: lane(),
            sub_oz: lane(),
            sub_sx: lane(),
            sub_sy: lane(),
            sub_sz: lane(),
        }
    }
}

/// A run set the mesher builds itself — a coarse tier's — with the same
/// lanes, owned.
#[derive(Default)]
pub struct OwnedRunSet {
    pub span: u32,
    pub start: Vec<u32>,
    pub z0: Vec<i32>,
    pub z1: Vec<i32>,
    pub layer_taps: Vec<u32>,
    pub r: Vec<u8>,
    pub g: Vec<u8>,
    pub b: Vec<u8>,
    pub material_class: Vec<u8>,
    pub sub_ox: Vec<u8>,
    pub sub_oy: Vec<u8>,
    pub sub_oz: Vec<u8>,
    pub sub_sx: Vec<u8>,
    pub sub_sy: Vec<u8>,
    pub sub_sz: Vec<u8>,
}

impl OwnedRunSet {
    /// Empties every lane, keeping their capacity.
    pub fn clear(&mut self) {
        self.start.clear();
        self.z0.clear();
        self.z1.clear();
        self.layer_taps.clear();
        for lane in [
            &mut self.r,
            &mut self.g,
            &mut self.b,
            &mut self.material_class,
            &mut self.sub_ox,
            &mut self.sub_oy,
            &mut self.sub_oz,
            &mut self.sub_sx,
            &mut self.sub_sy,
            &mut self.sub_sz,
        ] {
            lane.clear();
        }
    }

    pub fn view(&self) -> RunSet<'_> {
        RunSet {
            count: self.z0.len() as u32,
            span: self.span,
            start: &self.start,
            z0: &self.z0,
            z1: &self.z1,
            layer_taps: &self.layer_taps,
            r: &self.r,
            g: &self.g,
            b: &self.b,
            material_class: &self.material_class,
            sub_ox: &self.sub_ox,
            sub_oy: &self.sub_oy,
            sub_oz: &self.sub_oz,
            sub_sx: &self.sub_sx,
            sub_sy: &self.sub_sy,
            sub_sz: &self.sub_sz,
        }
    }
}
