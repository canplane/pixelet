// xpute-gpu-native-host/lib.rs

//! This is the host's end of a guest's GPU calls on a native device, as
//! gpu/web/host/src/host.ts is on the web. It runs a stream's records
//! (gpu/link/spec/api.json) on a wgpu device, and keeps each object the guest
//! made at its handle's slot. It knows nothing of what the guest draws.
//!
//! A request that finishes after its run, such as a read back, the queue's
//! work done or an async pipeline, is answered at a later `poll`. That holds
//! for an async pipeline too, which wgpu makes at once. So the guest finds its
//! ticket at the fence once the answer has landed, as on the web (spec
//! RESULT), and a run never waits for an answer.
//!
//! `native` holds the host for a guest linked into its own process, and
//! exports it under xpute's names (`native_abi!`).

pub mod abi;
mod admit;
mod belt;
mod cache;
mod configure;
mod image;
#[cfg(target_os = "android")]
pub mod jni;
mod make;
mod name;
#[cfg(unix)]
pub mod native;
mod object;
mod open;
mod pending;
mod run;
mod surface;
#[cfg(unix)]
pub mod vsync;
mod word;
mod worker;

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use xpute_link::mem;

use xpute_gpu_link::spec::device;
use xpute_gpu_link::spec::status;
use belt::Belt;
use cache::Cache;
use image::{Decoder, Decoding};
use make::{Maker, Making};
use name::EnumTable;
use object::Object;
use pending::Pending;
use surface::Surface;

pub use configure::ConfigureAt;
pub use open::{instance, open, Ask, OpenError, Wgpu};

#[cfg(test)]
use admit::FrameGuard;

/// The host runs by these: the program's config (`GpuConfig`) and the
/// platform's choices (`native::config`).
#[derive(Clone, Copy)]
pub struct Config {
    /// This is the format that SURFACE_PREFERRED stands for and the device's
    /// block tells the guest, as `getPreferredCanvasFormat` gives it on the web.
    pub preferred_format: wgpu::TextureFormat,
    pub configure_at: ConfigureAt,
    /// This is how many frames may be admitted and unfinished at once
    /// (`GpuHost::admit_frame`); at least one. It is also each surface's
    /// `desired_maximum_frame_latency`. A surface keeps one texture more than
    /// that, so taking a texture for an admitted frame never blocks.
    pub frame_latency: u32,
    /// This is the size of one chunk of the staging belt, chosen as large as a
    /// frame's writes; a larger write is staged across several chunks. The
    /// belt holds one chunk more than the frame latency.
    pub upload_chunk_sz: u64,
    /// A record, a run or a completion's landing slower than this is said
    /// (`GpuConfig::slow_ms`); with none, nothing is timed.
    pub slow_ms: Option<u32>,
    pub present_mode: wgpu::PresentMode,
}

pub struct GpuHost {
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
    /// This is the address where the guest asked for the device's block.
    block: Option<u32>,
    /// wgpu's device-lost callback sets this, on whatever thread wgpu calls it,
    /// and nothing clears it: a host runs one device (`lost`).
    device_lost: Arc<AtomicBool>,
    /// This counts the uncaptured errors the device raised. wgpu's handler adds
    /// to it, on whatever thread wgpu calls it, and `tell_counts` writes it.
    error_cnt: Arc<AtomicU32>,
    pending: Vec<Pending>,
    decoder: Decoder,
    maker: Maker,
    /// This is where the pipelines the maker compiles are kept across launches,
    /// where the device can (`Cache`).
    cache: Option<Cache>,
    making: HashMap<u32, Making>,
    decoding: VecDeque<Decoding>,
    /// These are a record's words, copied out of the guest's memory so that
    /// running the record may write that memory.
    scratch: Vec<u32>,
    cmd_bufs: Vec<wgpu::CommandBuffer>,
    /// This run's records have taken a surface's texture.
    drew: bool,
    /// This counts the frames this host drew that the queue has not yet
    /// finished. Each host keeps its own count, so no other host's frames hold
    /// back its own.
    unfinished_cnt: Arc<AtomicU32>,
    frame_latency: u32,
    slow_ms: Option<u32>,
    present_mode: wgpu::PresentMode,
    /// Every buffer write is staged through the belt and copied by
    /// `uploads`, which is submitted ahead of what follows.
    /// `Queue::write_buffer` would make a new Metal buffer for each write.
    belt: Belt,
    uploads: Option<wgpu::CommandEncoder>,
}

/// Runs `f`, and where `slow` is some and `f` took longer, says `[gpu] <what>
/// took <ms> ms`, the whole milliseconds, as a page's host says it
/// (`GpuConfig::slow_ms`, @xpute/gpu-web-host `timed`).
pub(crate) fn timed<R>(
    slow: Option<u32>,
    what: impl core::fmt::Display,
    f: impl FnOnce() -> R,
) -> R {
    let Some(slow) = slow else {
        return f();
    };
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_millis();
    if ms > u128::from(slow) {
        xpute_native_runtime::log(&format!("[gpu] {what} took {ms} ms"));
    }
    r
}

impl GpuHost {
    /// `wake` asks for a `poll` and may be called from any thread; beside a
    /// native guest it is native.rs's `wake`. Whatever learns of a completion
    /// calls it, so that the next `poll` writes the answer. A device the
    /// system loses (`Unknown`) is said, as a page's host says it, and told
    /// the guest at the next `poll`, and the host's owner lets the host go
    /// (`lost`). A device lost as `Destroyed` was its
    /// owner's doing, and is none of these.
    pub fn new(
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        cfg: Config,
        cache_dir: Option<&Path>,
        wake: fn(),
    ) -> GpuHost {
        assert!(
            cfg.frame_latency >= 1,
            "[gpu] a frame latency of {}",
            cfg.frame_latency
        );
        let device_lost = Arc::new(AtomicBool::new(false));
        let lost = device_lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            if reason == wgpu::DeviceLostReason::Unknown {
                xpute_native_runtime::log(&format!("[gpu] device lost ({reason:?}): {message}"));
                lost.store(true, Ordering::Release);
                wake();
            }
        });
        // An error no scope caught is logged and counted, as a page's host
        // does, in place of wgpu's default, which panics.
        let error_cnt = Arc::new(AtomicU32::new(0));
        let errors = error_cnt.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            xpute_native_runtime::log(&format!("[gpu] {e}"));
            errors.fetch_add(1, Ordering::Relaxed);
        }));
        let cache = cache_dir.and_then(|dir| Cache::open(&adapter, &device, dir));
        GpuHost {
            adapter,
            belt: Belt::new(&device, cfg.upload_chunk_sz, cfg.frame_latency + 1),
            device,
            queue,
            names: EnumTable::new(),
            objects: Vec::new(),
            surfaces: Vec::new(),
            preferred_format: cfg.preferred_format,
            configure_at: cfg.configure_at,
            block: None,
            device_lost,
            error_cnt,
            pending: Vec::new(),
            decoder: Decoder::new(wake),
            maker: make::maker(wake),
            cache,
            making: HashMap::new(),
            decoding: VecDeque::new(),
            scratch: Vec::new(),
            cmd_bufs: Vec::new(),
            drew: false,
            unfinished_cnt: Arc::new(AtomicU32::new(0)),
            frame_latency: cfg.frame_latency,
            slow_ms: cfg.slow_ms,
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

    /// This says whether the system lost the device. The host is then
    /// spent: once a `poll` has told the guest, its owner lets it go and opens
    /// a device anew, as a page asks for a new device.
    pub fn lost(&self) -> bool {
        self.device_lost.load(Ordering::Acquire)
    }

    /// Forgets every object of the device that went, and tells the guest in
    /// its block. The block is then let go: the next device's host writes it.
    fn on_lost(&mut self) {
        self.objects.clear();
        self.pending.clear();
        self.decoding.clear();
        self.making.clear();
        if let Some(addr) = self.block.take() {
            mem::store_release(addr + 4 * device::STATUS_AT as u32, status::LOST);
        }
    }

    /// Writes into `mem` what the device has answered since the last
    /// poll. Call it on the thread that runs the stream. After a loss it
    /// writes only the block's LOST STATUS: every object and pending request
    /// goes with the device. It says whether a request was answered.
    pub fn poll(&mut self) -> bool {
        if self.lost() {
            self.on_lost();
            return false;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.tell_counts();
        let waited = self.waiting();
        // Each completion's landing is timed as a page's is (`run::timed`);
        // the belt's chunks land in an atomic store, which has nothing to time.
        let slow = self.slow_ms;
        self.pending.retain(|p| p.poll(slow));
        self.land_images();
        self.land_pipelines();
        self.waiting() != waited
    }

    fn waiting(&self) -> usize {
        self.pending.len() + self.decoding.len() + self.making.len()
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod test;

#[cfg(test)]
#[path = "lib.noop.test.rs"]
mod noop;
