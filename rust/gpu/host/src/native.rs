// pxl-gpu-host/native.rs

//! This is the GPU's host for a guest on a native device, as web.ts is a
//! page's: the host serves `gpu` and the guest's `device` (spec/gpu/wit/gpu.wit)
//! on the host's side its runtime gives (`serve`), and binds the platform's windows to
//! the guest's surfaces (`bind`); `vsync` is vsync.rs's. The device, the
//! windows and everything run on them live on a thread of their own, the
//! GPU's (`on_gpu` says why): the records `gpu.run` lends are
//! run there and stay lent until its answer, the device is opened there when
//! the guest asks it ready, windows are bound there, and its completions land
//! there (`poll`). wgpu calls a mapping's or the queue's callbacks only as the
//! device is polled, so a thread of the device's own waits on its queue after
//! each run and wakes the GPU's thread as the work lands (`poller`), as a
//! browser fires them on a page, while the GPU's thread goes on. What the GPU
//! answers reaches the guest's thread as a task, by the request's job id, by
//! its own job (job.rs `job`, `complete`), over the way back the runtime gives
//! every host: this thread is the GPU's own, and the guest's thread is the
//! runtime's, which this host does not name. A device the system loses is let go with
//! the block it was lent, and the guest is told (`device.lost`); the guest asks
//! it ready again, as a page asks for a new device, and the next device binds
//! every window again. The platform is told what the host has of a GPU as it
//! changes (`state_on`), as a page's host reports it. There is one host a
//! process, as there is one guest.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_host::abi::log::Level;
use xpute_host::log::log;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Lent;
use xpute_link::{Return, Side};

use super::abi;
use super::job::{complete, job};
use super::vsync::Vsync;
use super::{Ask, Config, ConfigureAt, GpuHost, OpenError, Wgpu};
use pxl_gpu_link::GpuConfig;

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
            ndk_sys::ANativeWindow_acquire(window.as_ptr().cast());
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
            ndk_sys::ANativeWindow_release(self.0.as_ptr().cast());
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
    /// (`GpuHost::attach`) while it is lent: until a loss answers
    /// `device.lost` or its caller cancels it.
    block: Option<u64>,
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

/// This is the platform's function told what the host has of a GPU
/// (`state_on`): the state's name, its UTF-8 bytes and their count.
pub type State = extern "C" fn(state: *const u8, len: usize);

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// What the host has of a GPU now, the last state reported, none before the
/// guest's first ask is answered (`state`).
static CUR: Mutex<Option<&'static str>> = Mutex::new(None);

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
        upload_chunk_sz: cfg.upload_chunk_sz as u64,
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn device() -> MutexGuard<'static, Option<Device>> {
    DEVICE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Makes `state` be told what the host has of a GPU from the next change on,
/// as a page's host reports it (gpu.ts `GpuState`): `present` once the guest's
/// ask finds a device open, `missing` once it finds none opens, and `waiting`
/// once a device is lost and the guest told, as it asks again. With none,
/// nothing is told. It is called on the GPU's thread, and must not call into
/// this host.
pub fn state_on(state: Option<State>) {
    *STATE.lock().unwrap_or_else(PoisonError::into_inner) = state;
}

/// What the host has of a GPU now, as `state_on` last told it, or none
/// before the guest's first ask is answered: a UI that starts after a state
/// was told hears it so, as a reloaded page hears its new worker's host.
pub fn state() -> Option<&'static str> {
    *CUR.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reports the host's state to the platform (`state_on`), as web.ts
/// `report` does, and keeps it as the state now (`state`).
fn report(state: &'static str) {
    *CUR.lock().unwrap_or_else(PoisonError::into_inner) = Some(state);
    if let Some(f) = *STATE.lock().unwrap_or_else(PoisonError::into_inner) {
        f(state.as_ptr(), state.len());
    }
}

/// Runs `job` on the GPU's thread, which the first job starts. The GPU has a
/// thread of its own on a device because a present can wait on the frame
/// before, and the guest's thread must go on answering touches and
/// completions meanwhile, so the stream and the present run apart from it, as
/// a browser runs them outside the page.
fn on_gpu(job: Job) {
    let work = WORK.get_or_init(|| {
        let (work, jobs) = channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("pxl gpu".into())
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
/// go, its `lost` answered, and the platform told `waiting`.
fn poll() {
    let mut guard = device();
    let Some(d) = guard.as_mut() else { return };
    let Some(r) = d.host.as_mut() else { return };
    let lost = r.lost();
    let answered = r.poll();
    if answered || lost {
        for id in d.settled.drain(..) {
            complete(id, Ok::<(), Errno>(()));
        }
    }
    if lost {
        d.host = None;
        d.poller = None;
        d.block = None;
        for id in d.lost.drain(..) {
            complete(id, Ok::<(), Errno>(()));
        }
        drop(guard);
        report("waiting");
    }
}

/// Mounts `gpu` and `vsync` on `side`, the host's side its runtime gives, on
/// the guest's thread before it boots (a server the program lists), the
/// device `gpu` gives to be opened as `ask` and `cfg` say, its compiled
/// pipelines kept in `cache_dir`, the directory the platform keeps the app's
/// caches in.
pub fn serve(side: &'static Side, ask: Ask, cfg: Config, cache_dir: Option<PathBuf>) {
    let args = Arc::new(OpenArgs {
        ask,
        cfg,
        cache_dir,
    });
    abi::gpu::mount(side, Rc::new(Gpu { side, args }));
    abi::vsync::mount(side, Rc::new(Vsync));
}

/// What serves `gpu` on `side`, which issues the guest's device its handle.
struct Gpu {
    side: &'static Side,
    args: Arc<OpenArgs>,
}

impl abi::gpu::Host for Gpu {
    /// Runs the lent records on the GPU's thread, and answers once they are
    /// submitted, as a page's run resolves once they have run; with no
    /// device, unrun, as a page's host does. The frames they drew are
    /// presented after the answer.
    fn run(&self, recs: Lent, task_return: Return<()>) {
        let id = job(Box::new(move |()| task_return.answer(Ok(()))));
        on_gpu(Box::new(move || {
            let mut guard = device();
            let Some(d) = guard.as_mut() else {
                return complete(id, ());
            };
            let Some(r) = d.host.as_mut() else {
                return complete(id, ());
            };
            r.run(recs.at, recs.len);
            complete(id, ());
            r.present();
            if let Some(p) = &d.poller {
                let _ = p.send(());
            }
            drop(guard);
            poll();
        }));
    }

    /// The guest's device, served under a handle the link issues.
    fn get_device(&self) -> Result<u32, Errno> {
        let device = abi::gpu::device(self.side, Rc::new(GuestDevice(self.args.clone())));
        Ok(self.side.add(device))
    }
}

struct GuestDevice(Arc<OpenArgs>);

impl abi::gpu::HostDevice for GuestDevice {
    /// The block lent until the device is lost: written now where a device is
    /// open, and by each device opened while it is lent. A loss its caller
    /// cancels hands the block back, and the host writes it no more.
    fn lost(&self, block: Lent, task_return: Return<()>) {
        let id = wait(task_return, |d, id| {
            d.lost.retain(|&w| w != id);
            d.block = None;
            if let Some(r) = d.host.as_mut() {
                r.detach();
            }
        });
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

    /// Opens a device as the guest asks, once (`open`), or answers
    /// `no-device` where none opens; the platform is told which.
    fn ready(&self, task_return: Return<Result<(), abi::gpu::ErrorCode>>) {
        let id = job(Box::new(move |opened: Result<(), abi::gpu::ErrorCode>| {
            task_return.answer(Ok(opened));
        }));
        let args = self.0.clone();
        on_gpu(Box::new(move || {
            let opened = open(args);
            report(if opened.is_ok() { "present" } else { "missing" });
            complete(id, opened.map_err(|_| abi::gpu::ErrorCode::NoDevice));
        }));
    }

    fn settled(&self, task_return: Return<()>) {
        let id = wait(task_return, |d, id| d.settled.retain(|&w| w != id));
        let args = self.0.clone();
        on_gpu(Box::new(move || {
            device()
                .get_or_insert_with(|| new_device(&args.ask))
                .settled
                .push(id);
        }));
    }
}

/// The job of a request that waits in one of the device's lists until it is
/// told (`poll`), or answers ECANCELED once its caller cancels it, as
/// @pxl/gpu/host web.ts `wait` does: on the GPU's thread `cancel` takes
/// it out of its list and stops what else it holds, and then it answers.
fn wait(task_return: Return<()>, cancel: fn(&mut Device, u64)) -> u64 {
    let id = Rc::new(Cell::new(0));
    task_return.on_cancel({
        let id = id.clone();
        move || {
            let id = id.get();
            on_gpu(Box::new(move || {
                if let Some(d) = device().as_mut() {
                    cancel(d, id);
                }
                complete(id, Err::<(), Errno>(Errno::ECANCELED));
            }));
        }
    });
    id.set(job(Box::new(move |r: Result<(), Errno>| {
        task_return.answer(r);
    })));
    id.get()
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
/// is logged through the host's log, its surface drawn into textures never
/// shown until the next device.
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
    } = super::open(&d.instance, &args.ask)?;
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
                Err(e) => log(
                    Level::Error,
                    "gpu",
                    &format!("no surface of surface {handle:#x}'s window on this device: {e}"),
                ),
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
        .name("pxl gpu poll".into())
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
/// the window, it is logged and the window let go; where a newer handle has
/// bound the slot since, the window is let go. While no device is open the
/// window is kept, and the next device binds it (`open`). The host holds a
/// reference of its own on a window bound until it is unbound (`Window`).
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
                return log(
                    Level::Error,
                    "gpu",
                    &format!("no surface of {handle:#x}'s window"),
                );
            };
            if !h.bind(handle, surface) {
                return;
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
/// - `pxl_gpu_state_on(void (*state)(const char *state, size_t len))`
///   makes `state` be told what the host has of a GPU as it changes, by its
///   name's UTF-8 bytes and their count (`state_on`); null sets none.
/// - `pxl_gpu_state(size_t *len) -> const char *` answers what the host has
///   of a GPU now, its name's UTF-8 bytes, their count put at `len`, or null
///   before the guest's first ask is answered (`state`).
/// - `pxl_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`
///   binds the guest's surface `handle` to `window` (`bind`): 0, or EINVAL
///   for a null window. It exists on an Apple platform and on Android, whose
///   windows `target` takes.
/// - `pxl_gpu_surface_unbind(uint32_t handle)` lets the window go
///   (`unbind`).
/// - `pxl_gpu_vsync()` is the display's frame, from its display link
///   (vsync.rs `vsync`).
///
/// On Android the same calls are exported to JNI for the Kotlin class
/// `me.plei.pixelet.gpu.Gpu` over the JNI helpers the assembly names by `$jni`
/// (`jni_abi!`).
#[macro_export]
macro_rules! native_abi {
    ($($jni:tt)+) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn pxl_gpu_state_on(state: Option<$crate::native::State>) {
            $crate::native::state_on(state);
        }

        /// # Safety
        /// `len` is a writable word.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn pxl_gpu_state(len: *mut usize) -> *const u8 {
            let Some(s) = $crate::native::state() else {
                return ::core::ptr::null();
            };
            // SAFETY: the caller's word.
            unsafe { *len = s.len() };
            s.as_ptr()
        }

        /// # Safety
        /// `window` is null or a live window of the platform's kind.
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn pxl_gpu_surface_bind(
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

        #[unsafe(no_mangle)]
        pub extern "C" fn pxl_gpu_surface_unbind(handle: u32) {
            $crate::native::unbind(handle);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn pxl_gpu_vsync() {
            $crate::vsync::vsync();
        }

        #[cfg(target_os = "android")]
        $crate::jni_abi!($($jni)+);
    };
}

#[cfg(test)]
#[path = "native.test.rs"]
mod test;
