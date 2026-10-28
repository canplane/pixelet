// pxl-render/gpu/ray/tally.rs

//! What the eye's cast and the sun's walked and took, counted where the
//! `ray_counts` setting asks (pxl-spec-core `tally`): where each pass
//! counts cleared each counted frame, gathered into one array after the
//! frame's dispatches where no mapping of it is out (`copied`), mapped back
//! (`mapped`), and summed as it lands with what the host knew of the frame
//! that counted it (`Frame`), since boot.

use pxl_spec_core::{observed, raster, ray, tally};
use xpute_guest::mem::off_of;

use crate::gpu::api::{buffer_usage, bytes, word_bytes, Gpu, DONE};

const WORDS: usize = tally::WORDS as usize;

/// What the profiler is handed (`Tally::sums`), summed since boot, by
/// index: the frames whose counts landed (`FRAMES`); then an observer's
/// each (`sum_at`): of them, those it cast in (`CASTS`), its pixels or
/// slots in them (`RAYS`), and its lanes (pxl-spec-core `tally`, from
/// `LANES_AT`).
pub const FRAMES: usize = 0;
pub const CASTS: usize = 0;
pub const RAYS: usize = 1;
pub const LANES_AT: usize = 2;
const OBSERVER_SUMS: usize = LANES_AT + tally::LANES as usize;
pub const RAY_SUMS: usize = 1 + tally::OBSERVERS as usize * OBSERVER_SUMS;

/// Where `observer`'s sum `k` is.
pub const fn sum_at(observer: u32, k: usize) -> usize {
    1 + observer as usize * OBSERVER_SUMS + k
}

/// What the host knew of a counted frame, an observer's each: whether it
/// cast, and its pixels or slots.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub casts: [bool; tally::OBSERVERS as usize],
    pub rays: [u32; tally::OBSERVERS as usize],
}

/// Where the passes counted, an observer's each: the walks' in the observed
/// pyramid's head (`seen`), the pixels' or slots' in their walk again's
/// args (`args`), the boxes in the raster's draw (`boxes`).
pub(super) struct Counted {
    pub seen: u32,
    pub args: [u32; tally::OBSERVERS as usize],
    pub boxes: [u32; tally::OBSERVERS as usize],
}

/// The array and its readback; whether this frame counts (`on`) and what
/// the host knew of the frame copied (`copied_frame`).
pub(super) struct Tally {
    counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copied_frame: Frame,
    landed: Box<[u32; WORDS]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
}

/// The sums since boot (`Tally::sums`): beside the tally, so a lost device's
/// next cast sums on.
static SUMS: xpute_guest::global::Global<[f64; RAY_SUMS]> = xpute_guest::global::Global::new();

impl Tally {
    /// Its buffers, made with the cast's: a few words.
    pub(super) fn new(g: &Gpu) -> Tally {
        Tally {
            counts: g.buffer(
                word_bytes(WORDS),
                buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            ),
            read: g.buffer(
                word_bytes(WORDS),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            on: false,
            frame: Frame::default(),
            copied_frame: Frame::default(),
            landed: Box::new([0; WORDS]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
        }
    }

    /// This frame's counting where `on` and an observer casts (`frame`):
    /// the walks told to count in the pyramid's head (`seen`) and their
    /// counts cleared there, after the head is written, and the array
    /// cleared.
    /// The head is left as it is where neither this frame nor the one
    /// before counts.
    pub(super) fn begin(&mut self, g: &Gpu, on: bool, seen: u32, frame: Frame) {
        let was = self.on;
        self.on = on && frame.casts.contains(&true);
        if !self.on && !was {
            return;
        }
        let mut head = [0u32; 1 + (tally::OBSERVERS * tally::WALKS) as usize];
        head[0] = u32::from(self.on);
        g.write_buffer(seen, word_bytes(observed::TALLY_ON as usize), bytes(&head));
        if !self.on {
            return;
        }
        self.frame = frame;
        g.write_buffer(self.counts, 0, bytes(&[0u32; WORDS]));
    }

    /// The counts gathered into the array and copied where none are out,
    /// after the frame's dispatches: each observer's that cast.
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32, from: &Counted) {
        if !self.on || self.copied || self.mapped {
            return;
        }
        for o in 0..tally::OBSERVERS {
            let k = o as usize;
            if !self.frame.casts[k] {
                continue;
            }
            // Each run where it was counted, into its lanes.
            for (buffer, word, lane, words) in [
                (
                    from.seen,
                    observed::tally_at(o, tally::BEAM),
                    tally::BEAM,
                    tally::WALKS,
                ),
                (
                    from.args[k],
                    ray::MISS_ARGS_TALLY,
                    tally::PICKED,
                    tally::RAYS,
                ),
                (from.boxes[k], raster::ARGS_INSTANCES, tally::BOXES, 1),
            ] {
                let to = word_bytes(tally::at(o, lane) as usize);
                let size = word_bytes(words as usize);
                g.copy_buffer(
                    encoder,
                    (buffer, word_bytes(word as usize)),
                    (self.counts, to),
                    size,
                );
            }
        }
        g.copy_buffer(encoder, (self.counts, 0), (self.read, 0), word_bytes(WORDS));
        (self.copied, self.copied_frame) = (true, self.frame);
    }

    /// The counts copied last frame mapped back, and those that landed
    /// summed with what the host knew of their frame: after the frame that
    /// copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        if self.mapped && *self.mapping & DONE != 0 {
            let (l, f) = (&self.landed, self.copied_frame);
            let s = SUMS.get(|| [0.0; RAY_SUMS]);
            s[FRAMES] += 1.0;
            for o in 0..tally::OBSERVERS {
                if !f.casts[o as usize] {
                    continue;
                }
                s[sum_at(o, CASTS)] += 1.0;
                s[sum_at(o, RAYS)] += f64::from(f.rays[o as usize]);
                for lane in 0..tally::LANES {
                    let c = f64::from(l[tally::at(o, lane) as usize]);
                    s[sum_at(o, LANES_AT + lane as usize)] += c;
                }
            }
            self.mapped = false;
        }
        if self.copied && *self.mapping & DONE != 0 {
            let address = off_of(self.landed.as_ptr()) as u32;
            g.map_read(
                self.read,
                0,
                word_bytes(WORDS),
                address,
                &mut self.mapping,
                1,
            );
            (self.copied, self.mapped) = (false, true);
        }
    }

    /// Its sums since boot (`RAY_SUMS`).
    pub(super) fn sums() -> [f64; RAY_SUMS] {
        *SUMS.get(|| [0.0; RAY_SUMS])
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn on_dropped(&mut self) {
        self.copied = false;
    }
}
