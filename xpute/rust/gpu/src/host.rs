// xpute-gpu/host.rs

//! The host's end of a guest's GPU calls on a native device, the counterpart
//! of ts/gpu/src/host.ts: runs a stream's records (spec/gpu/api.json) on a
//! wgpu device, each object at its handle's slot. It knows nothing of what
//! the guest draws.
//!
//! What finishes after a turn, a read back or the queue's work done, lands
//! at a later turn's `poll`: the guest finds `ticket | DONE` a turn or more
//! later, as on the web, and nothing here waits.

mod admit;
mod configure;
mod image;
mod name;
mod object;
mod open;
mod pending;
mod run;
mod surface;
mod word;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use xpute_host::mem::Memory;

use crate::spec::STATUS;
use image::{Decoder, Decoding};
use name::Names;
use object::Object;
use pending::Pending;
use surface::Surface;

pub use configure::ConfigureAt;
pub use open::{open, Ask, OpenError, Opened};

#[cfg(test)]
use crate::spec::DONE;
#[cfg(test)]
use admit::Unfinished;

/// The program's numbers and choices, which the host runs by: none of them
/// is the host's to pick.
#[derive(Clone, Copy)]
pub struct Plan {
    /// The format SURFACE_PREFERRED names and DEVICE tells the guest, as the
    /// web's `getPreferredCanvasFormat`.
    pub preferred: wgpu::TextureFormat,
    pub configure_at: ConfigureAt,
    /// The frames admitted and unfinished at once (`GpuHost::admit`), and
    /// each surface's `desired_maximum_frame_latency`; at least one. A
    /// surface keeps one texture more, so taking one never blocks.
    pub frame_latency: u32,
    /// The staging belt's chunk: as large as a frame's writes, since a
    /// larger write takes a chunk of its own.
    pub upload_chunk_bytes: u64,
    pub present_mode: wgpu::PresentMode,
}

pub struct GpuHost {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    names: Names,
    objects: Vec<Option<Object>>,
    /// At the number the program binds each by.
    surfaces: Vec<Surface>,
    preferred: wgpu::TextureFormat,
    configure_at: ConfigureAt,
    /// Where the guest asked the device's block written.
    block: Option<(u32, u32)>,
    /// Set from wgpu's thread; taken at `poll`.
    gone: Arc<AtomicBool>,
    pending: Vec<Pending>,
    decoder: Decoder,
    decoding: VecDeque<Decoding>,
    /// A record's words, copied out so the record may write the memory.
    scratch: Vec<u32>,
    finished: Vec<wgpu::CommandBuffer>,
    /// This turn's frame was admitted, so it is counted once presented.
    admitted: bool,
    /// This host's own count, so another host's frames, or a gone device's,
    /// hold back none of its own.
    unfinished: Arc<AtomicU32>,
    frame_latency: u32,
    present_mode: wgpu::PresentMode,
    /// Every buffer write goes through the belt into `uploads`, submitted
    /// ahead of what follows: `Queue::write_buffer` makes a Metal buffer a write.
    belt: wgpu::util::StagingBelt,
    uploads: Option<wgpu::CommandEncoder>,
}

impl GpuHost {
    /// `wake` asks the platform's loop for a turn, from any thread: whoever
    /// learns of a completion between turns asks for the turn that writes it.
    /// A device lost (`Unknown`, not `Destroyed`, which is its owner's doing)
    /// is told the guest at the next turn's edge.
    pub fn new(
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        plan: Plan,
        wake: fn(),
    ) -> GpuHost {
        assert!(
            plan.frame_latency >= 1,
            "[gpu] a frame latency of {}",
            plan.frame_latency
        );
        let gone = Arc::new(AtomicBool::new(false));
        let lost = gone.clone();
        device.set_device_lost_callback(move |reason, _| {
            if reason == wgpu::DeviceLostReason::Unknown {
                lost.store(true, Ordering::Release);
                wake();
            }
        });
        GpuHost {
            adapter,
            belt: wgpu::util::StagingBelt::new(device.clone(), plan.upload_chunk_bytes),
            device,
            queue,
            names: Names::new(),
            objects: Vec::new(),
            surfaces: Vec::new(),
            preferred: plan.preferred,
            configure_at: plan.configure_at,
            block: None,
            gone,
            pending: Vec::new(),
            decoder: Decoder::new(wake),
            decoding: VecDeque::new(),
            scratch: Vec::new(),
            finished: Vec::new(),
            admitted: false,
            unfinished: Arc::new(AtomicU32::new(0)),
            frame_latency: plan.frame_latency,
            present_mode: plan.present_mode,
            uploads: None,
        }
    }

    fn lost(&mut self, memory: &mut Memory<'_>) {
        self.objects.clear();
        self.pending.clear();
        self.decoding.clear();
        if let Some((address, words)) = self.block {
            memory.set_word(address + 4 * (words - 1), STATUS::LOST);
        }
    }

    /// What the device has answered since, written into `memory`: run at the
    /// turn's edge, before the turn reads its words. A loss ends everything else.
    pub fn poll(&mut self, memory: &mut Memory<'_>) {
        if self.gone.swap(false, Ordering::AcqRel) {
            self.lost(memory);
            return;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.pending.retain(|p| p.poll(memory));
        self.land_images(memory);
    }
}

#[cfg(test)]
#[path = "host.test.rs"]
mod test;
