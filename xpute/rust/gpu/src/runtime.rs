// xpute-gpu/runtime.rs

//! The runtime's end of a core's GPU calls on a native device, the counterpart
//! of ts/gpu/src/runtime.ts: runs a stream's records (spec/gpu/api.json) on a
//! wgpu device, each object at its handle's slot. It knows nothing of what
//! the core draws.
//!
//! What finishes after a turn, a read back, the queue's work done or an
//! async pipeline, which wgpu makes at once, lands at a later turn's `poll`:
//! the core finds its ticket at the fence a turn or more later (spec
//! RESULT), as on the web, and nothing here waits.
//!
//! Beside a core linked into its own process, `native` holds the runtime and
//! exports it under xpute's names (`native_abi!`).

mod admit;
mod belt;
mod configure;
mod image;
mod name;
#[cfg(unix)]
pub mod native;
mod object;
mod open;
mod pending;
mod run;
mod surface;
mod word;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use xpute_runtime::mem::Memory;

use crate::device;
use crate::spec::STATUS;
use belt::Belt;
use image::{Decoder, Decoding};
use name::Names;
use object::Object;
use pending::Pending;
use surface::Surface;

pub use configure::ConfigureAt;
pub use open::{open, Ask, OpenError, Opened};

#[cfg(test)]
use admit::FrameGuard;

/// The program's numbers and choices, which the runtime runs by.
#[derive(Clone, Copy)]
pub struct Config {
    /// The format SURFACE_PREFERRED names and DEVICE tells the core, as the
    /// web's `getPreferredCanvasFormat`.
    pub preferred_format: wgpu::TextureFormat,
    pub configure_at: ConfigureAt,
    /// The frames admitted and unfinished at once (`GpuRuntime::admit_frame`), and
    /// each surface's `desired_maximum_frame_latency`; at least one. A
    /// surface keeps one texture more, so taking one never blocks.
    pub frame_latency: u32,
    /// The staging belt's chunk: as large as a frame's writes, a larger
    /// write staged across several. The belt holds one more chunk than the
    /// frame latency.
    pub upload_chunk_bytes: u64,
    pub present_mode: wgpu::PresentMode,
}

pub struct GpuRuntime {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    names: Names,
    objects: Vec<Option<Object>>,
    /// At the number the program binds each by.
    surfaces: Vec<Surface>,
    preferred_format: wgpu::TextureFormat,
    configure_at: ConfigureAt,
    /// Where the core asked the device's block written.
    block: Option<u32>,
    /// Set from wgpu's thread; taken at `poll`.
    gone: Arc<AtomicBool>,
    pending: Vec<Pending>,
    decoder: Decoder,
    decoding: VecDeque<Decoding>,
    /// A record's words, copied out so the record may write the memory.
    scratch: Vec<u32>,
    command_bufs: Vec<wgpu::CommandBuffer>,
    /// This turn's frame was admitted, so it is counted once presented.
    admitted: bool,
    /// This runtime's own count, so another runtime's frames, or a gone device's,
    /// hold back none of its own.
    unfinished_cnt: Arc<AtomicU32>,
    frame_latency: u32,
    present_mode: wgpu::PresentMode,
    /// Every buffer write goes through the belt into `uploads`, submitted
    /// ahead of what follows: `Queue::write_buffer` makes a Metal buffer a write.
    belt: Belt,
    uploads: Option<wgpu::CommandEncoder>,
}

impl GpuRuntime {
    /// `wake` asks for a turn, from any thread (beside a native core, its
    /// kick): whoever learns of a completion asks for the turn whose `poll`
    /// writes it.
    /// A device lost (`Unknown`, not `Destroyed`, which is its owner's doing)
    /// is told the core at the next turn's edge.
    pub fn new(
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        cfg: Config,
        wake: fn(),
    ) -> GpuRuntime {
        assert!(
            cfg.frame_latency >= 1,
            "[gpu] a frame latency of {}",
            cfg.frame_latency
        );
        let gone = Arc::new(AtomicBool::new(false));
        let lost = gone.clone();
        device.set_device_lost_callback(move |reason, _| {
            if reason == wgpu::DeviceLostReason::Unknown {
                lost.store(true, Ordering::Release);
                wake();
            }
        });
        GpuRuntime {
            adapter,
            belt: Belt::new(&device, cfg.upload_chunk_bytes, cfg.frame_latency + 1),
            device,
            queue,
            names: Names::new(),
            objects: Vec::new(),
            surfaces: Vec::new(),
            preferred_format: cfg.preferred_format,
            configure_at: cfg.configure_at,
            block: None,
            gone,
            pending: Vec::new(),
            decoder: Decoder::new(wake),
            decoding: VecDeque::new(),
            scratch: Vec::new(),
            command_bufs: Vec::new(),
            admitted: false,
            unfinished_cnt: Arc::new(AtomicU32::new(0)),
            frame_latency: cfg.frame_latency,
            present_mode: cfg.present_mode,
            uploads: None,
        }
    }

    /// The device the stream runs on, for the program's own calls on the
    /// thread that runs the stream: an error scope pushed there holds what
    /// the records run after it raise.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn on_lost(&mut self, memory: &mut Memory<'_>) {
        self.objects.clear();
        self.pending.clear();
        self.decoding.clear();
        if let Some(address) = self.block {
            memory.store_release(address + 4 * device::STATUS_AT as u32, STATUS::LOST);
        }
    }

    /// What the device has answered since, written into `memory`: run on the
    /// thread that runs the stream, before a turn reads its words. A loss
    /// ends everything else.
    pub fn poll(&mut self, memory: &mut Memory<'_>) {
        if self.gone.swap(false, Ordering::AcqRel) {
            self.on_lost(memory);
            return;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.tell_unfinished(memory);
        self.pending.retain(|p| p.poll(memory));
        self.land_images(memory);
    }
}

#[cfg(test)]
#[path = "runtime.test.rs"]
mod test;

#[cfg(test)]
#[path = "runtime.noop.test.rs"]
mod noop;
