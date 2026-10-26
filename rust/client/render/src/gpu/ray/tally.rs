// pxl-render/gpu/ray/tally.rs

//! What the sun's cast walked and took, counted where the `sun_counts`
//! setting asks (pxl-spec-core `sun::COUNTS`; pxl-shader sun.rs): the GPU's
//! counts cleared each counted frame, copied where no mapping of them is
//! out (`copied`), mapped back (`mapped`), and summed as they land with
//! what the host knew of the frame that counted them (`Frame`), since boot.

use pxl_spec_core::sun;
use xpute_guest::mem::off_of;

use crate::gpu::api::{buffer_usage, bytes, word_bytes, Gpu, DONE};

/// What the profiler is handed (`Tally::sums`), summed since boot, by
/// index: the frames whose counts landed; of them, those that cast the
/// sun's map, every slot of it; the map's slots in the frames that cast it;
/// the slots that took the raster's pick; the boxes drawn (pxl-spec-core
/// `sun::COUNT_PICKED` on).
pub const FRAMES: usize = 0;
pub const SUN_FRAMES: usize = 1;
pub const TEXELS: usize = 2;
pub const PICKED: usize = 3;
pub const BOXES: usize = 4;
pub const SUN_SUMS: usize = 5;

/// What the host knew of a counted frame: whether it cast the sun's map,
/// and the map's slots.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub sun: bool,
    pub texels: u32,
}

/// The counts and their readback; whether this frame counts (`on`) and
/// what the host knew of the frame copied (`copied_frame`).
pub(super) struct Tally {
    pub(super) counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copied_frame: Frame,
    landed: Box<[u32; sun::COUNTS as usize]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
    sums: [f64; SUN_SUMS],
}

impl Tally {
    /// Its buffers, made with the cast's: a few words.
    pub(super) fn new(g: &Gpu) -> Tally {
        Tally {
            counts: g.buffer(
                word_bytes(sun::COUNTS as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            ),
            read: g.buffer(
                word_bytes(sun::COUNTS as usize),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            on: false,
            frame: Frame::default(),
            copied_frame: Frame::default(),
            landed: Box::new([0; sun::COUNTS as usize]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
            sums: [0.0; SUN_SUMS],
        }
    }

    /// This frame's counting, where `on` and it casts the sun's map
    /// (`frame`): the counts cleared.
    pub(super) fn begin(&mut self, g: &Gpu, on: bool, frame: Frame) {
        self.on = on && frame.sun;
        if !self.on {
            return;
        }
        self.frame = frame;
        g.write_buffer(self.counts, 0, bytes(&[0u32; sun::COUNTS as usize]));
    }

    /// The counts copied where none are out, after the frame's dispatches.
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32) {
        if self.on && !self.copied && !self.mapped {
            let size = word_bytes(sun::COUNTS as usize);
            g.copy_buffer(encoder, (self.counts, 0), (self.read, 0), size);
            (self.copied, self.copied_frame) = (true, self.frame);
        }
    }

    /// The counts copied last frame mapped back, and those that landed
    /// summed with what the host knew of their frame: after the frame that
    /// copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        if self.mapped && *self.mapping & DONE != 0 {
            let (l, f, s) = (&self.landed, self.copied_frame, &mut self.sums);
            let c = |k: u32| f64::from(l[k as usize]);
            s[FRAMES] += 1.0;
            if f.sun {
                s[SUN_FRAMES] += 1.0;
                s[TEXELS] += f64::from(f.texels);
                s[PICKED] += c(sun::COUNT_PICKED);
                s[BOXES] += c(sun::COUNT_BOXES);
            }
            self.mapped = false;
        }
        if self.copied && *self.mapping & DONE != 0 {
            let address = off_of(self.landed.as_ptr()) as u32;
            let size = word_bytes(sun::COUNTS as usize);
            g.map_read(self.read, 0, size, address, &mut self.mapping, 1);
            (self.copied, self.mapped) = (false, true);
        }
    }

    /// Its sums since boot (`SUN_SUMS`).
    pub(super) fn sums(&self) -> [f64; SUN_SUMS] {
        self.sums
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn dropped(&mut self) {
        self.copied = false;
    }
}
