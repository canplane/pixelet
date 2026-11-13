// xpute-gpu-native-host/native.rs

//! This is the GPU's host for a guest on a native device, as web.ts is a
//! page's: the host serves `gpu` and the guest's `device` (gpu/wit/gpu.wit)
//! on xpute-native-host's link (`serve`), and binds the platform's windows to
//! the guest's surfaces (`bind`); `vsync` is vsync.rs's. The device, the
//! windows and everything run on them live on a thread of their own, the
//! GPU's, since a present waits on the frame before it and would hold the
//! guest's loop or the platform's main thread: the records `gpu.run` lends are
//! run there and stay lent until its answer, the device is opened there when
//! the guest asks it ready, windows are bound there, and its completions land
//! there (`poll`). wgpu calls a mapping's or the queue's callbacks only as the
//! device is polled, so a thread of the device's own waits on its queue after
//! each run and wakes the GPU's thread as the work lands (`poller`), as a
//! browser fires them on a page, while the GPU's thread goes on. What the GPU
//! answers reaches the guest's thread as a task, by the request's job id
//! (xpute-native-host `job`, `done`). A device the system loses is let go with
//! the block it was lent, the guest is told (`device.lost`), and the platform
//! too (`lost_on`); the guest asks it ready again, as a page asks for a new
//! device, and the next device binds every window again. There is one host a
//! process, as there is one guest.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_link::abi::types::{Lent, Token};
use xpute_kit::status::errno::Errno;
use xpute_link::{answer, Return};
use xpute_native_host::link;
use xpute_native_host::{done, job};

use super::abi;
use super::vsync::Vsync;
use super::{Ask, Config, ConfigureAt, GpuHost, OpenError, Wgpu};
use xpute_gpu_link::spec::STATUS;
use xpute_gpu_link::GpuConfig;

/// This is a platform's window, as `target` takes it, with a reference of the
/// host's own on it from its bind until its unbind: on an Apple platform a
/// view's `CAMetalLayer`, retained, and on Android a view's native window,
/// acquired. Each device opened makes its surface of the window anew.
#[cfg(any(target_vendor = "apple", target_os = "android"))]
struct Window(NonNull<c_void>);

// SAFETY: a counted reference, which may be taken and let go on any thread.
#[cfg(any(target_vendor = "apple", target_os = "android"))]
unsafe impl Send for Window {}

#[cfg(target_vendor = "apple")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

#[cfg(any(target_vendor = "apple", target_os = "android"))]
impl Window {
    /// Takes a reference on `window`.
    ///
    /// # Safety
    /// `window` is a live window of the platform's kind.
    unsafe fn of(window: NonNull<c_void>) -> Window {
        // SAFETY: a live window, which the reference keeps until it drops.
        unsafe {
            #[cfg(target_vendor = "apple")]
            CFRetain(window.as_ptr());
            #[cfg(target_os = "android")]
            xpute_native_host::jni::ndk::ANativeWindow_acquire(window.as_ptr().cast());
        }
        Window(window)
    }
}

#[cfg(any(target_vendor = "apple", target_os = "android"))]
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the reference `of` took.
        unsafe {
            #[cfg(target_vendor = "apple")]
            CFRelease(self.0.as_ptr());
            #[cfg(target_os = "android")]
            xpute_native_host::jni::ndk::ANativeWindow_release(self.0.as_ptr().cast());
        }
    }
}

/// What a device is opened by: the program's ask and config, and where the
/// platform keeps compiled pipelines (`serve`), as a page's `WebGpu` holds
/// its own.
struct OpenArgs {
    ask: Ask,
    cfg: Config,
    cache_dir: Option<PathBuf>,
}

struct Device {
    /// Each window's surface is made with this, on every device opened.
    instance: wgpu::Instance,
    /// This is the open device's host, and none from a loss until a device
    /// opens again.
    host: Option<GpuHost>,
    /// This tells the open device's poller that a run was submitted.
    poller: Option<Sender<()>>,
    /// Where the guest lent the device's block, which each device writes
    /// (`GpuHost::attach`), until a loss answers `device.lost`.
    block: Option<u32>,
    /// The requests waiting on the device, by job id: the next completion's
    /// (`device.settled`), and its loss (`device.lost`).
    settled: Vec<u64>,
    lost: Vec<u64>,
}

/// The device, held by the GPU's thread and by an Apple platform's main
/// thread as it configures (`CONFIGURE_AT`). It holds a device once the guest
/// first asks of it.
static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

#[cfg(any(target_vendor = "apple", target_os = "android"))]
thread_local! {
    /// These are the windows bound, by the guest's handle, in the order they
    /// were bound, so a device opened again binds each slot's newest handle
    /// last. They are the only place a bound window is held, on the GPU's
    /// thread.
    static WINDOWS: RefCell<Vec<(u32, Window)>> = const { RefCell::new(Vec::new()) };
}

type Job = Box<dyn FnOnce() + Send>;

/// The GPU's thread's queue of work, in the order it was asked for.
static WORK: OnceLock<Sender<Job>> = OnceLock::new();

/// This is the platform's function told of each device lost (`lost_on`).
pub type Lost = extern "C" fn();

static LOST: Mutex<Option<Lost>> = Mutex::new(None);

/// A native guest's surfaces are configured on the main thread of an Apple
/// platform, and elsewhere where the stream runs.
#[cfg(target_vendor = "apple")]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Here;

/// The host prefers BGRA on an Apple platform and RGBA elsewhere, since
/// Android's Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// This is the host's config for a native guest: the program's `GpuConfig`,
/// the platform's preferred format, where it configures a surface
/// (`CONFIGURE_AT`), and `Fifo`, which shows each frame in turn at the
/// display's pace and is the one present mode every platform has.
pub const fn config(cfg: GpuConfig) -> Config {
    Config {
        preferred_format: PREFERRED_FORMAT,
        configure_at: CONFIGURE_AT,
        frame_latency: cfg.frame_latency,
        upload_chunk_bytes: cfg.upload_chunk_bytes,
        slow_ms: cfg.slow_ms,
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn device() -> MutexGuard<'static, Option<Device>> {
    DEVICE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Makes `lost` be told of each device lost from the next one on; with none,
/// nothing is told. It is called once a loss, on the GPU's thread, after the
/// guest has been told, and must not call into this host.
pub fn lost_on(lost: Option<Lost>) {
    *LOST.lock().unwrap_or_else(PoisonError::into_inner) = lost;
}

/// Runs `job` on the GPU's thread, which the first job starts.
fn on_gpu(job: Job) {
    let work = WORK.get_or_init(|| {
        let (work, jobs) = channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("xpute gpu".into())
            .spawn(move || jobs.into_iter().for_each(|job| job()));
        if spawned.is_err() {
            xpute_kit::bug!(EAGAIN);
        }
        work
    });
    let _ = work.send(job);
}

/// What wgpu's callbacks and the host's workers call when an answer may
/// have landed, from any thread (`GpuHost::new`'s `wake`): the GPU's
/// thread polls.
fn wake() {
    on_gpu(Box::new(poll));
}

/// On the GPU's thread: lands what the device has answered, and answers the
/// guest's waiting `settled` where something did. A device found lost is let
/// go, its `lost` answered, and the platform told.
fn poll() {
    let mut guard = device();
    let Some(d) = guard.as_mut() else { return };
    let Some(r) = d.host.as_mut() else { return };
    let lost = r.lost();
    let answered = r.poll();
    if answered || lost {
        for id in d.settled.drain(..) {
            done(id, 0);
        }
    }
    if lost {
        d.host = None;
        d.poller = None;
        d.block = None;
        for id in d.lost.drain(..) {
            done(id, 0);
        }
        drop(guard);
        if let Some(f) = *LOST.lock().unwrap_or_else(PoisonError::into_inner) {
            f();
        }
    }
}

/// Mounts `gpu`, the guest's device and `vsync` on the host's link, on the
/// guest's thread before it boots (the program composes it), a device to
/// be opened as `ask` and `cfg` say, its compiled pipelines kept in
/// `cache_dir`, the directory the platform keeps the app's caches in.
pub fn serve(ask: Ask, cfg: Config, cache_dir: Option<PathBuf>) {
    let args = Arc::new(OpenArgs {
        ask,
        cfg,
        cache_dir,
    });
    abi::gpu::mount(Rc::new(Gpu));
    let device = xpute_link::uri(&xpute_gpu_link::DEVICE_URI);
    link::mount(device, move |_: &[Token], _| {
        Ok(abi::gpu::device(Rc::new(GuestDevice(args.clone()))))
    });
    abi::vsync::mount(Rc::new(Vsync));
}

struct Gpu;

impl abi::gpu::Gpu for Gpu {
    /// Runs the lent records on the GPU's thread, and answers once they are
    /// submitted, as a page's run resolves once they have run; with no
    /// device, unrun, as a page's host does. The frames they drew are
    /// presented after the answer.
    fn run(&self, records: Lent, task_return: Return<()>) {
        let id = job(Box::new(move |r| task_return.answer(answer(r).map(drop))));
        on_gpu(Box::new(move || {
            let mut guard = device();
            let Some(d) = guard.as_mut() else {
                return done(id, 0);
            };
            let Some(r) = d.host.as_mut() else {
                return done(id, 0);
            };
            r.run(records.at, records.len);
            done(id, 0);
            r.present();
            if let Some(p) = &d.poller {
                let _ = p.send(());
            }
            drop(guard);
            poll();
        }));
    }
}

struct GuestDevice(Arc<OpenArgs>);

impl abi::gpu::Device for GuestDevice {
    /// The block lent until the device is lost: written now where a device is
    /// open, and by each device opened while it is lent.
    fn lost(&self, block: Lent, task_return: Return<()>) {
        let id = job(Box::new(move |r| task_return.answer(answer(r).map(drop))));
        let args = self.0.clone();
        on_gpu(Box::new(move || {
            let mut guard = device();
            let d = guard.get_or_insert_with(|| new_device(&args.ask));
            d.block = Some(block.at);
            if let Some(r) = d.host.as_mut() {
                r.attach(block.at);
            }
            d.lost.push(id);
        }));
    }

    /// Opens a device as the guest asks, once (`open`): READY, or ENODEV where
    /// none opens.
    fn ready(&self, task_return: Return<u32>) {
        let id = job(Box::new(move |r| task_return.answer(answer(r))));
        let args = self.0.clone();
        on_gpu(Box::new(move || {
            let result = open(args);
            done(
                id,
                result.map_or(Errno::ENODEV as i32, |()| STATUS::READY as i32),
            );
        }));
    }

    fn settled(&self, task_return: Return<()>) {
        let id = job(Box::new(move |r| task_return.answer(answer(r).map(drop))));
        let args = self.0.clone();
        on_gpu(Box::new(move || {
            device()
                .get_or_insert_with(|| new_device(&args.ask))
                .settled
                .push(id);
        }));
    }
}

fn new_device(ask: &Ask) -> Device {
    Device {
        instance: super::instance(ask),
        host: None,
        poller: None,
        block: None,
        settled: Vec::new(),
        lost: Vec::new(),
    }
}

#[cfg(target_vendor = "apple")]
fn configure() {
    if let Some(r) = device().as_mut().and_then(|d| d.host.as_mut()) {
        r.configure_surfaces();
    }
}

/// Opens a device as `args` says, on the GPU's thread, keeping its compiled
/// pipelines in its cache directory where one is given (`Cache`). A device
/// already open is kept. A new device writes the block lent, and binds every
/// window bound, in the order bound; one the system makes no surface of now
/// is logged, its surface drawn into textures never shown until the next
/// device.
fn open(args: Arc<OpenArgs>) -> Result<(), OpenError> {
    let mut guard = device();
    let d = guard.get_or_insert_with(|| new_device(&args.ask));
    if d.host.is_some() {
        return Ok(());
    }
    let Wgpu {
        adapter,
        device,
        queue,
    } = super::open(&d.instance, &args.ask).inspect_err(|e| {
        xpute_native_host::log(&format!("[gpu] the device did not open: {e:?}"));
    })?;
    d.poller = Some(poller(device.clone()));
    let host = GpuHost::new(
        adapter,
        device,
        queue,
        args.cfg,
        args.cache_dir.as_deref(),
        wake,
    );
    let host = d.host.insert(host);
    if let Some(block) = d.block {
        host.attach(block);
    }
    #[cfg(any(target_vendor = "apple", target_os = "android"))]
    WINDOWS.with_borrow(|windows| {
        for (handle, window) in windows {
            // SAFETY: a window the host holds a reference on.
            match unsafe { d.instance.create_surface_unsafe(target(window.0)) } {
                Ok(surface) => {
                    host.bind(*handle, surface);
                }
                Err(e) => xpute_native_host::log(&format!(
                    "[gpu] no surface of surface {handle:#x}'s window on this device: {e}"
                )),
            }
        }
    });
    Ok(())
}

/// Starts `device`'s poller: after each run it is told of, it waits until the
/// queue's work submitted so far is done, which runs the callbacks of what
/// landed, and wakes the GPU's thread to land it. It ends with the device's
/// host, which drops its sender.
fn poller(device: wgpu::Device) -> Sender<()> {
    let (tell, told) = channel::<()>();
    let spawned = std::thread::Builder::new()
        .name("xpute gpu poll".into())
        .spawn(move || {
            while told.recv().is_ok() {
                while told.try_recv().is_ok() {}
                let _ = device.poll(wgpu::PollType::wait_indefinitely());
                wake();
            }
        });
    if spawned.is_err() {
        xpute_kit::bug!(EAGAIN);
    }
    tell
}

/// This is the target wgpu draws a platform's window through: on an Apple
/// platform a view's `CAMetalLayer`, on Android a view's native window.
#[cfg(target_vendor = "apple")]
fn target(window: NonNull<c_void>) -> wgpu::SurfaceTargetUnsafe {
    wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(window.as_ptr())
}

#[cfg(target_os = "android")]
fn target(window: NonNull<c_void>) -> wgpu::SurfaceTargetUnsafe {
    use wgpu::rwh::{
        AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle,
    };
    wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(window)),
    }
}

/// Binds the guest's surface `handle` to `window` on the GPU's thread, on the
/// open device and on every device opened after it, and returns at once, so
/// the caller never waits on a present. Where the system makes no surface of
/// the window, or a newer handle has bound the slot since, it is said and the
/// window let go. While no device is open the window is kept, and the next
/// device binds it (`open`). The host holds a reference of its own on a
/// window bound until it is unbound (`Window`).
///
/// # Safety
/// `window` is a live window of the platform's kind (`target`).
#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub unsafe fn bind(handle: u32, window: NonNull<c_void>) {
    // SAFETY: the caller's live window, which `Window` holds from here.
    let window = unsafe { Window::of(window) };
    on_gpu(Box::new(move || {
        let mut guard = device();
        let d = guard.as_mut();
        if let Some((d, h)) = d.and_then(|d| d.host.as_mut().map(|h| (&d.instance, h))) {
            // SAFETY: the window `Window` holds.
            let Ok(surface) = (unsafe { d.create_surface_unsafe(target(window.0)) }) else {
                return xpute_native_host::log(&format!("[gpu] no surface of {handle:#x}'s window"));
            };
            if !h.bind(handle, surface) {
                return xpute_native_host::log(&format!("[gpu] {handle:#x} is bound over"));
            }
        }
        drop(guard);
        WINDOWS.with_borrow_mut(|w| {
            w.retain(|&(bound, _)| bound != handle);
            w.push((handle, window));
        });
    }));
}

/// Lets `handle`'s window go on the GPU's thread: its surface, if `handle` is
/// still the one bound in its slot (`GpuHost::unbind`), and then the
/// host's reference on the window `handle` bound, whatever the slot holds
/// now. It returns once the window is let go, so nothing draws into it after,
/// as Android asks of a surface it takes back.
pub fn unbind(handle: u32) {
    let (gone, wait) = channel::<()>();
    on_gpu(Box::new(move || {
        if let Some(h) = device().as_mut().and_then(|d| d.host.as_mut()) {
            h.unbind(handle);
        }
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        WINDOWS.with_borrow_mut(|w| w.retain(|&(bound, _)| bound != handle));
        let _ = gone.send(());
    }));
    let _ = wait.recv();
}

/// Exports the GPU's host under xpute's names, for the platform:
///
/// - `xpute_gpu_lost_on(void (*lost)(void))` makes `lost` be told of each
///   device lost, once a loss (`lost_on`); null sets none.
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`
///   binds the guest's surface `handle` to `window` (`bind`): 0, or EINVAL
///   for a null window. It exists on an Apple platform and on Android, whose
///   windows `target` takes.
/// - `xpute_gpu_surface_unbind(uint32_t handle)` lets the window go
///   (`unbind`).
/// - `xpute_gpu_vsync()` is the display's frame, from its display link
///   (vsync.rs `vsync`).
///
/// On Android the same calls are exported to JNI for the Kotlin class
/// `xpute.Gpu` (`jni_abi!`).
#[macro_export]
macro_rules! native_abi {
    () => {
        #[no_mangle]
        pub extern "C" fn xpute_gpu_lost_on(lost: Option<$crate::native::Lost>) {
            $crate::native::lost_on(lost);
        }

        /// # Safety
        /// `window` is null or a live window of the platform's kind.
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        #[no_mangle]
        pub unsafe extern "C" fn xpute_gpu_surface_bind(
            handle: u32,
            window: *mut ::core::ffi::c_void,
        ) -> i32 {
            let Some(window) = ::core::ptr::NonNull::new(window) else {
                return ::xpute_kit::status::errno::Errno::EINVAL as i32;
            };
            // SAFETY: the caller's live window.
            unsafe { $crate::native::bind(handle, window) };
            0
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_surface_unbind(handle: u32) {
            $crate::native::unbind(handle);
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_vsync() {
            $crate::vsync::vsync();
        }

        #[cfg(target_os = "android")]
        $crate::jni_abi!();
    };
}
