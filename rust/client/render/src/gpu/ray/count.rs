// pxl-render/gpu/ray/count.rs

//! What the eye's cast and the light's walked and reused, counted where the
//! `ray_counts` setting asks (pxl-engine `count`): where each pass
//! counts cleared each counted frame, gathered into one array after the
//! frame's dispatches where no mapping of it is out (`copied`), mapped back
//! (`mapped`), and handed on as it lands with what the CPU knew of the
//! frame that counted it (`Frame`), by the renderer's number of that frame
//! (`RayCounts`), as a GPU read is.

use pxl_engine::{count, pyramid, raster, ray};
use xpute_guest::mem::off_of;

use crate::gpu::api::{buffer_usage, bytes, word_bytes, Gpu, DONE};

const WORDS: usize = count::WORDS as usize;

/// A counted frame's counts, by index, an observer's each (`count_at`):
/// whether it cast, 1 or 0 (`CAST`), its pixels or slots (`RAYS`), and its
/// lanes (pxl-engine `count`, from `LANES_AT`).
pub const CAST: usize = 0;
pub const RAYS: usize = 1;
pub const LANES_AT: usize = 2;
const OBSERVER_COUNTS: usize = LANES_AT + count::LANES as usize;
pub const RAY_COUNTS: usize = count::OBSERVERS as usize * OBSERVER_COUNTS;

/// Where `observer`'s count `k` is.
pub const fn count_at(observer: u32, k: usize) -> usize {
    observer as usize * OBSERVER_COUNTS + k
}

/// One frame's counts as they land, a frame or two after it: the renderer's
/// number of it (renderer.rs `FrameStat::frame`) and its counts
/// (`count_at`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayCounts {
    pub frame: f64,
    pub counts: [f64; RAY_COUNTS],
}

/// What the CPU knew of a counted frame: the renderer's number it will
/// take (renderer.rs `FrameStat::frame`), and an observer's each, whether it
/// cast and its pixels or slots.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub number: f64,
    pub casts: [bool; count::OBSERVERS as usize],
    pub rays: [u32; count::OBSERVERS as usize],
}

/// Where the passes counted, an observer's each: the walks' in the observed
/// pyramid's head (`pyramid`), the pixels' or slots' in their walk again's
/// args (`args`), the boxes in the raster's draw (`boxes`).
pub(super) struct Counts {
    pub pyramid: u32,
    pub args: [u32; count::OBSERVERS as usize],
    pub boxes: [u32; count::OBSERVERS as usize],
}

/// The array and its readback; whether this frame counts (`on`), what
/// the CPU knew of the frame copied (`copied_frame`), and the counts that
/// landed, until taken (`take`).
pub(super) struct Counter {
    counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copied_frame: Frame,
    read_counts: Box<[u32; WORDS]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
    ray_counts: Option<RayCounts>,
}

impl Counter {
    /// Its buffers, made with the cast's: a few words.
    pub(super) fn new(g: &Gpu) -> Counter {
        Counter {
            counts: g.buf(
                word_bytes(WORDS),
                buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            ),
            read: g.buf(
                word_bytes(WORDS),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            on: false,
            frame: Frame::default(),
            copied_frame: Frame::default(),
            read_counts: Box::new([0; WORDS]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
            ray_counts: None,
        }
    }

    /// This frame's counting where `on` and an observer casts (`frame`):
    /// the walks told to count in the pyramid's head (`pyramid`) and their
    /// counts cleared there, after the head is written, and the array
    /// cleared.
    /// The head is left as it is where neither this frame nor the one
    /// before counts.
    pub(super) fn begin(&mut self, g: &Gpu, on: bool, pyramid: u32, frame: Frame) {
        let was = self.on;
        self.on = on && frame.casts.contains(&true);
        if !self.on && !was {
            return;
        }
        let mut head = [0u32; 1 + (count::OBSERVERS * count::WALKS) as usize];
        head[0] = u32::from(self.on);
        g.write_buf(
            pyramid,
            word_bytes(pyramid::COUNT_ON as usize),
            bytes(&head),
        );
        if !self.on {
            return;
        }
        self.frame = frame;
        g.write_buf(self.counts, 0, bytes(&[0u32; WORDS]));
    }

    /// The counts gathered into the array and copied where none are out,
    /// after the frame's dispatches: each observer's that cast.
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32, from: &Counts) {
        if !self.on || self.copied || self.mapped {
            return;
        }
        for o in 0..count::OBSERVERS {
            let k = o as usize;
            if !self.frame.casts[k] {
                continue;
            }
            // Each run where it was counted, into its lanes.
            for (buf, word, lane, words) in [
                (
                    from.pyramid,
                    pyramid::lane_at(o, count::BEAM),
                    count::BEAM,
                    count::WALKS,
                ),
                (
                    from.args[k],
                    ray::MISS_ARGS_COUNT,
                    count::REUSED,
                    count::RAYS,
                ),
                (from.boxes[k], raster::ARGS_INSTANCES, count::BOXES, 1),
            ] {
                let to = word_bytes(count::at(o, lane) as usize);
                let size = word_bytes(words as usize);
                g.copy_buf(
                    encoder,
                    (buf, word_bytes(word as usize)),
                    (self.counts, to),
                    size,
                );
            }
        }
        g.copy_buf(encoder, (self.counts, 0), (self.read, 0), word_bytes(WORDS));
        (self.copied, self.copied_frame) = (true, self.frame);
    }

    /// The counts copied last frame mapped back, and those that landed
    /// kept with what the CPU knew of their frame, until taken: after the
    /// frame that copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        if self.mapped && *self.mapping & DONE != 0 {
            let (l, f) = (&self.read_counts, self.copied_frame);
            let mut counts = [0.0; RAY_COUNTS];
            for o in 0..count::OBSERVERS {
                if !f.casts[o as usize] {
                    continue;
                }
                counts[count_at(o, CAST)] = 1.0;
                counts[count_at(o, RAYS)] = f64::from(f.rays[o as usize]);
                for lane in 0..count::LANES {
                    let c = f64::from(l[count::at(o, lane) as usize]);
                    counts[count_at(o, LANES_AT + lane as usize)] = c;
                }
            }
            self.ray_counts = Some(RayCounts {
                frame: f.number,
                counts,
            });
            self.mapped = false;
        }
        if self.copied && *self.mapping & DONE != 0 {
            let addr = off_of(self.read_counts.as_ptr()) as u32;
            g.map_read(self.read, 0, word_bytes(WORDS), addr, &mut self.mapping, 1);
            (self.copied, self.mapped) = (false, true);
        }
    }

    /// The counts that landed since last taken.
    pub(super) fn take(&mut self) -> Option<RayCounts> {
        self.ray_counts.take()
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn on_dropped(&mut self) {
        self.copied = false;
    }
}
