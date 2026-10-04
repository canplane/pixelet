// pxl-kernel/layer/earth.rs

//! The Earth's picture as a layer of the pyramid: only its top zoom is baked,
//! and only the floor's pages about the eye are held, so it brings no demand,
//! zoom or schedule of its own. A page's texels go to the GPU once and are
//! kept nowhere here; the global picture answers wherever no page is held.

use pxl_spec::bake::earth_page::{self, EARTH_Z};
use pxl_spec::space::lod::{fault_radius, squares_within, FAULT_ACROSS, FAULT_PAGES};
use pxl_spec::space::pyramid::{page_at, Folds, Page};
use xpute_runtime::global::Global;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{free, Block};

use crate::io::demand::{Category, Phases};
use crate::sched::profiler::record::Failure;
use crate::world::paging::Paging;
use crate::world::pyramid::{Layer, Pyramid};

/// What a held page is: its texels are in its frame's layer on the GPU.
#[derive(Clone)]
pub struct Earth;

impl Folds for Earth {
    /// Only the top zoom is baked, so nothing is folded.
    fn fold_page<'a>(_p: Page, _held: impl Fn(Page) -> Option<&'a Self>) -> Option<Self> {
        None
    }
}

static EARTH: Global<Pyramid<Earth>> = Global::new();

impl Layer for Earth {
    const CATEGORY: Category = Category::earth;
    const DEMANDS: bool = false;
    const TOP: Option<u32> = Some(EARTH_Z);

    fn pyramid() -> &'static mut Pyramid<Earth> {
        EARTH.get(Pyramid::default)
    }

    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<Earth, Failure> {
        earth_page::decode(pkt)?;
        Ok(Earth)
    }
}

/// Per frame, the packet whose texels are not yet on the GPU; a frame taken
/// again replaces it.
static UPLOADS: Global<Vec<Option<Block>>> = Global::new();

fn uploads() -> &'static mut Vec<Option<Block>> {
    UPLOADS.get(|| vec![None; Paging::<Earth>::RECORDS as usize])
}

fn view<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: a body's block, kept here alone until it is freed.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at), bytes as usize) }
}

fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, released once.
        unsafe { free(block) };
    }
}

/// The packet is kept until its texels are uploaded, or freed if no frame takes it.
pub fn adopt(p: Page, block: Block, phases: &mut Phases) -> Result<(), Failure> {
    let mut framed = None;
    let result = Earth::pyramid().admit(p, |record| {
        let page = Earth::receive(record, view(block), phases)?;
        framed = Some(record);
        Ok(page)
    });
    match framed {
        Some(record) => release(uploads()[record as usize].replace(block)),
        None => release(Some(block)),
    }
    result
}

pub fn earth_step() {
    Earth::pyramid().step(None);
}

/// Each waiting packet's texels to `write`, into its frame's layer, while it
/// takes them.
pub fn earth_upload(write: &mut dyn FnMut(u32, &[u8]) -> bool) {
    for (record, slot) in uploads().iter_mut().enumerate() {
        let Some(block) = *slot else { continue };
        let Ok(texels) = earth_page::decode(view(block)) else { continue };
        if !write(record as u32 - 1, texels) {
            return;
        }
        release(slot.take());
    }
}

/// The floor's window of pages at the top, its south-west page first, and per
/// page row by row its layer plus one, zero where the global picture answers:
/// residency as the GPU reads it, kept nowhere.
pub fn earth_window() -> ([i32; 2], [u32; FAULT_PAGES as usize]) {
    let mut layers = [0; FAULT_PAGES as usize];
    let pyramid = Earth::pyramid();
    let Some(o) = pyramid.observation() else { return ([0; 2], layers) };
    let squares = || squares_within(EARTH_Z, [o.eye[0], o.eye[1]], fault_radius(EARTH_Z));
    let x0 = squares().map(|[x, _]| x).min().unwrap_or(0);
    let y0 = squares().map(|[_, y]| y).min().unwrap_or(0);
    for [x, y] in squares() {
        let (dx, dy) = ((x - x0) as u32, (y - y0) as u32);
        let Some(record) = pyramid.paging().frame(page_at(EARTH_Z, [x, y])) else { continue };
        if dx < FAULT_ACROSS && dy < FAULT_ACROSS && uploads()[record as usize].is_none() {
            layers[(dy * FAULT_ACROSS + dx) as usize] = record;
        }
    }
    let p = page_at(EARTH_Z, [x0, y0]);
    ([pxl_spec::space::pvf3::ivec::X(p.page) as i32, pxl_spec::space::pvf3::ivec::Y(p.page) as i32], layers)
}

/// The frames the GPU keeps a layer for.
pub const EARTH_FRAMES: u32 = Paging::<Earth>::FRAMES;
