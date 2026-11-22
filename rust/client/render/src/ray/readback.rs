// pxl-render/ray/readback.rs

//! Reads back a ray's words (pxl-engine ray.rs) at one drawn pixel of the world
//! surface's `rays`, for whoever asks what the pixel draws. The read of a
//! pixel (`RayPixel`) is what asks: it answers the words it holds of that
//! pixel, and where those are not the last frame's it copies the pixel's words
//! out, in an encoder of the copy's own, and asks its mapping with it, in the
//! reader's entry and never in a frame's stream. One readback is out at a
//! time, and a read while one is out copies nothing. The words land an entry
//! or two later, named by the renderer's number of the frame that left them,
//! and a later read answers them. No frame is asked for a read.

use pxl_engine::ray;
use pxl_gpu_guest::encode::Id;
use xpute_link::mem;

use crate::gpu::api::{Gpu, Mapping, buffer_usage, place, word_sz};

const WORDS: usize = ray::WORDS as usize;

/// The words a layer of the copy takes in the buffer: a row of a layered copy
/// is 256 bytes, as WebGPU aligns it, and each layer is one row.
const ROW_WORDS: usize = 64;

/// A drawn pixel of the world surface, column then row with row 0 at the top,
/// whose ray's words are read back, and `ray`, the ray's origin then its
/// direction, handed back as given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayPixel {
    pub px: [u32; 2],
    pub ray: [f64; 6],
}

/// A pixel's ray's words as they landed. `frame` is the renderer's number of
/// the last frame encoded when they were copied (renderer.rs
/// `FrameStat::frame`), so the words are as that frame left them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayWords {
    pub frame: f64,
    pub at: RayPixel,
    pub words: [u32; WORDS],
}

/// The readback's buffer and its state. `out` is the copy that is out, its
/// mapping asked, until that mapping lands: the frame and the pixel its words
/// are of, or none where the rays they were copied from are gone (`forget`).
/// `ray_words` holds the last words that landed.
pub(super) struct Readback {
    read: u32,
    out: Option<Option<(f64, RayPixel)>>,
    read_words: Box<[u32; ROW_WORDS * WORDS]>,
    mapping: Mapping,
    ray_words: Option<RayWords>,
}

impl Readback {
    pub(super) fn new(g: &Gpu) -> Readback {
        Readback {
            read: g.buf(
                word_sz(ROW_WORDS * WORDS),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            out: None,
            read_words: Box::new([0; ROW_WORDS * WORDS]),
            mapping: Mapping::default(),
            ray_words: None,
        }
    }

    /// The words that landed of pixel `at`, once the mapping that landed is
    /// taken in. Where they are not of frame `drawn`, the last encoded, and
    /// no copy is out, the pixel's words are copied out of `to`'s rays, in an
    /// encoder of its own submitted here, and the copy's mapping is asked;
    /// `drawn` names them. `to` is none where the rays may not be copied.
    pub(super) fn ray_words(
        &mut self,
        to: Option<(&Gpu, Id)>,
        drawn: f64,
        at: RayPixel,
    ) -> Option<RayWords> {
        if self.out.is_some()
            && let Some(ok) = self.mapping.landed()
            && let Some(Some((frame, at))) = self.out.take()
            && ok
        {
            let words = core::array::from_fn(|k| self.read_words[k * ROW_WORDS]);
            self.ray_words = Some(RayWords { frame, at, words });
        }
        let words = self.ray_words.filter(|w| w.at.px == at.px);
        if words.is_none_or(|w| w.frame != drawn)
            && self.out.is_none()
            && let Some((g, rays)) = to
        {
            let encoder = g.encoder();
            g.copy_texel_to_buf(
                encoder,
                &place(rays, [at.px[0], at.px[1], 0]),
                self.read,
                word_sz(ROW_WORDS),
                ray::WORDS,
            );
            g.submit(encoder);
            let addr = mem::addr(&*self.read_words);
            self.mapping
                .ask(g, self.read, word_sz(ROW_WORDS * WORDS), addr);
            self.out = Some(Some((drawn, at)));
        }
        words
    }

    /// Lets go of the words that landed and of those of the copy that is
    /// out: the `rays` they are of are gone.
    pub(super) fn forget(&mut self) {
        self.ray_words = None;
        if let Some(of) = &mut self.out {
            *of = None;
        }
    }
}
