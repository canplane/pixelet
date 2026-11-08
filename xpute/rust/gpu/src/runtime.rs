// xpute-gpu/runtime.rs

//! This is the runtime's end of a core's GPU calls on a native device, as
//! ts/gpu/src/runtime.ts is on the web. It runs a stream's records
//! (spec/gpu/api.json) on a wgpu device, and keeps each object the core made
//! at its handle's slot. It knows nothing of what the core draws.
//!
//! A request that finishes after its turn, such as a read back, the queue's
//! work done or an async pipeline, is answered at a later turn's `poll`. That
//! holds for an async pipeline too, which wgpu makes at once. So the core
//! finds its ticket at the fence a turn or more later, as on the web (spec
//! RESULT), and the runtime never waits for an answer.
//!
//! `native` holds the runtime for a core linked into its own process, and
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
use name::EnumTable;
use object::Object;
use pending::Pending;
use surface::Surface;

pub use configure::ConfigureAt;
pub use open::{open, Ask, OpenError, Opened};

#[cfg(test)]
use admit::FrameGuard;

/// The runtime runs by these: the program's config (`CoreConfig`) and the
/// platform's choices (`native::config`).
#[derive(Clone, Copy)]
pub struct Config {
    /// This is the format that SURFACE_PREFERRED stands for and the device's
    /// block tells the core, as `getPreferredCanvasFormat` gives it on the web.
    pub preferred_format: wgpu::TextureFormat,
    pub configure_at: ConfigureAt,
    /// This is how many frames may be admitted and unfinished at once
    /// (`GpuRuntime::admit_frame`); at least one. It is also each surface's
    /// `desired_maximum_frame_latency`. A surface keeps one texture more than
    /// that, so taking a texture for an admitted frame never blocks.
    pub frame_latency: u32,
    /// This is the size of one chunk of the staging belt, chosen as large as a
    /// frame's writes; a larger write is staged across several chunks. The
    /// belt holds one chunk more than the frame latency.
    pub upload_chunk_bytes: u64,
    pub present_mode: wgpu::PresentMode,
}

pub struct GpuRuntime {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    names: EnumTable,
    objects: Vec<Option<Object>>,
    /// This is indexed by surface number, and holds the slot of the handle the
    /// program binds the surface by.
    surfaces: Vec<Surface>,
    preferred_format: wgpu::TextureFormat,
    configure_at: ConfigureAt,
    /// This is the address where the core asked for the device's block.
    block: Option<u32>,
    /// wgpu's device-lost callback sets this, on whatever thread wgpu calls it,
    /// and `poll` takes it.
    gone: Arc<AtomicBool>,
    pending: Vec<Pending>,
    decoder: Decoder,
    decoding: VecDeque<Decoding>,
    /// These are a record's words, copied out of the core's memory so that
    /// running the record may write that memory.
    scratch: Vec<u32>,
    command_bufs: Vec<wgpu::CommandBuffer>,
    /// This turn's frame was admitted, so `present` counts it unfinished.
    admitted: bool,
    /// This counts the frames this runtime admitted that the queue has not yet
    /// finished. Each runtime keeps its own count, so no other runtime's frames
    /// hold back its own.
    unfinished_cnt: Arc<AtomicU32>,
    frame_latency: u32,
    present_mode: wgpu::PresentMode,
    /// Every buffer write is staged through the belt and copied by
    /// `uploads`, which is submitted ahead of what follows.
    /// `Queue::write_buffer` would make a new Metal buffer for each write.
    belt: Belt,
    uploads: Option<wgpu::CommandEncoder>,
}

impl GpuRuntime {
    /// `wake` asks for a turn and may be called from any thread; beside a
    /// native core it is the core's kick. Whatever learns of a completion
    /// calls it, so that the next turn's `poll` writes the answer. A device
    /// the system loses (`Unknown`) is told the core before the next turn. A
    /// device lost as `Destroyed` was its owner's doing, and is not told.
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
            names: EnumTable::new(),
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

    /// This is the device the stream runs on, for the program's own calls on
    /// the thread that runs the stream. An error scope the program pushes there
    /// catches the errors of the records run after it.
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

    /// Writes into `memory` what the device has answered since the last
    /// poll. Call it on the thread that runs the stream, before each turn
    /// reads its words. After a loss it writes only the block's LOST STATUS:
    /// every object and pending request goes with the device.
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
