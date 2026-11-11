// xpute-gpu/runtime/native.rs

//! This is the GPU's runtime for a core linked into its own process
//! (xpute-runtime native), and its C ABI under xpute's names (`native_abi!`).
//! On Android the Kotlin class `xpute.Gpu` reaches it too (`jni`). The device
//! is opened as the app asks and held until the system loses it. Then the
//! runtime lets it go, keeps the instance and the windows bound, and tells
//! the app (`lost_on`), which opens a device again, as a page asks for a new
//! device: the next device binds every window again. A platform's window is
//! bound to a core's surface and let go. A published stream runs beside the
//! core, and a completion asks for a turn.
//!
//! No turn waits on the device. `publish` hands its work over and returns, and
//! `poll` skips a turn the device is busy for. An async pipeline is compiled
//! apart from the stream, and a sync one holds it (xpute-core io/stream.rs):
//! the core goes on turning and records nothing until END comes back. A frame's
//! present waits behind the stream, and holds only the streams published after
//! it. There is one runtime a
//! process, as there is one core, so its exports carry no program's prefix.

use core::ffi::{c_char, c_void, CStr};
use core::ptr::NonNull;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_runtime::mem::Memory;
use xpute_runtime::native::Core;

use super::admit::Admission;
use super::{Ask, Config, ConfigureAt, GpuRuntime, OpenError, Wgpu};
use crate::CoreConfig;

/// This is a platform's window, as `target` takes it, with a reference of the
/// runtime's own on it from its bind until its unbind: on an Apple platform a
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
            xpute_runtime::jni::ndk::ANativeWindow_acquire(window.as_ptr().cast());
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
            xpute_runtime::jni::ndk::ANativeWindow_release(self.0.as_ptr().cast());
        }
    }
}

struct Device {
    /// Each window's surface is made with this, on every device opened.
    instance: wgpu::Instance,
    /// These are the windows bound, by the core's handle, in the order they
    /// were bound, so a device opened again binds each slot's newest handle
    /// last. They are the only place a bound window is held.
    #[cfg(any(target_vendor = "apple", target_os = "android"))]
    windows: Vec<(u32, Window)>,
    /// This is the open device's runtime, and none from a loss until a device
    /// opens again.
    runtime: Option<GpuRuntime>,
}

/// Whoever uses the runtime holds this, so a surface is bound or configured
/// only while no stream runs. It holds a device once `open` is first called.
static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

type Job = Box<dyn FnOnce() + Send>;

/// The device's queue of work, in the order the core asked for it. It exists
/// once a device has opened, and outlives every device.
static WORK: OnceLock<Sender<Job>> = OnceLock::new();

/// The open device's admission, read as the core asks (`admit_frame`), and
/// none while no device is open.
static ADMISSION: Mutex<Option<Admission>> = Mutex::new(None);

/// This is the platform's function told of each device lost (`lost_on`).
pub type Lost = extern "C" fn();

static LOST: Mutex<Option<Lost>> = Mutex::new(None);

/// A native core's surfaces are configured on the main thread of an Apple
/// platform, and elsewhere where the stream runs.
#[cfg(target_vendor = "apple")]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Here;

/// The runtime prefers BGRA on an Apple platform and RGBA elsewhere, since
/// Android's Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// This is the runtime's config for a native core: the program's `numbers`, the
/// platform's preferred format, where it configures a surface
/// (`CONFIGURE_AT`), and `Fifo`, which shows each frame in turn at the
/// display's pace and is the one present mode every platform has.
pub const fn config(cfg: CoreConfig) -> Config {
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

fn admission() -> MutexGuard<'static, Option<Admission>> {
    ADMISSION.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Makes `lost` be told of each device lost from the next one on; with none,
/// nothing is told. It is called once a loss, on the thread that found it,
/// after the runtime has written the loss into the core's device block and let
/// the device go, so the next `open` opens a new one. It must not call into
/// this runtime: it hands the news to the app's thread, which opens a device
/// again.
pub fn lost_on(lost: Option<Lost>) {
    *LOST.lock().unwrap_or_else(PoisonError::into_inner) = lost;
}

impl Device {
    /// Writes into `mem` what the open device has answered (`GpuRuntime::poll`),
    /// and says whether a request was answered. A device found lost is let go
    /// once the poll has told the core, and the platform is told (`lost_on`).
    /// The loss is read before the poll, which then sees it too, since a loss
    /// is never cleared.
    fn poll(&mut self, mem: &mut Memory<'_>) -> bool {
        let Some(h) = self.runtime.as_mut() else {
            return false;
        };
        let lost = h.lost();
        let answered = h.poll(mem);
        if lost {
            self.runtime = None;
            *admission() = None;
            let lost = *LOST.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(f) = lost {
                f();
            }
        }
        answered
    }
}

/// Runs `f` on the runtime while a device is open, for the program's own
/// calls. It waits for a stream that is running.
pub fn with_runtime<R>(f: impl FnOnce(&mut GpuRuntime) -> R) -> Option<R> {
    device().as_mut().and_then(|d| d.runtime.as_mut()).map(f)
}

/// Runs `f` where the stream runs, after all the work the core has asked for
/// so far, while the caller waits. An error scope the program pushes there
/// catches the errors of the records run after it. Before a device has first
/// opened it answers none.
pub fn on_stream<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> Option<R> {
    let (done, answer) = channel();
    let job: Job = Box::new(move || {
        let _ = done.send(f());
    });
    WORK.get()?.send(job).ok()?;
    answer.recv().ok()
}

#[cfg(target_vendor = "apple")]
fn configure() {
    with_runtime(|h| h.configure_surfaces());
}

/// This is the core's memory as the runtime reads and writes it.
fn view(core: Core) -> Memory<'static> {
    let (base, bytes) = core.mem();
    // SAFETY: the memory, reserved for the process's life. The runtime reads
    // and writes only what the core has handed it, the stream and what its
    // records name until END comes back (xpute-core io/stream.rs R3) and the
    // words the runtime alone answers (R4), and each access reaches only its
    // own range (xpute-runtime mem.rs).
    unsafe { Memory::from_raw(base, bytes) }
}

/// Writes into the core's memory what the device has answered since the last
/// turn, so that the turn about to run reads it. It runs before each turn and
/// never waits: while a stream runs, that stream polls as it ends.
pub fn poll(core: Core) {
    let Ok(mut guard) = DEVICE.try_lock() else {
        return;
    };
    if let Some(d) = guard.as_mut() {
        d.poll(&mut view(core));
    }
}

/// Takes the stream at `stream_at`, which the core has just published by
/// storing END with release (xpute-core `BootInfo::publish`). The records run on
/// the device and END is stored back to 0 with release, as a page's runtime
/// does, and only then are the frames shown, since showing one can wait for
/// the frame before it. While no device is open, before the first or after a
/// loss, the stream is handed back unrun, as a page's runtime does, so the
/// core is never held and no record waits for a device that opens later.
pub fn publish(core: Core, stream_at: u32) {
    let Some(work) = WORK.get() else {
        super::run::release(&mut view(core), stream_at);
        return;
    };
    let job: Job = Box::new(move || {
        let mut mem = view(core);
        let mut guard = device();
        let answered = guard.as_mut().and_then(|d| {
            let h = d.runtime.as_mut()?;
            h.run(&mut mem, stream_at);
            h.present();
            Some(d.poll(&mut mem))
        });
        drop(guard);
        match answered {
            None => super::run::release(&mut mem, stream_at),
            Some(true) => xpute_runtime::native::wake(),
            Some(false) => {}
        }
    });
    let _ = work.send(job);
}

/// This is the runtime's admission (`crate::admit`), which `core!` gives the
/// program. It admits a frame while a device is open and fewer than the frame
/// latency are unfinished, as a page's admits one only while its device is
/// READY (`crate::admit::page`). While no device is open, before the first or
/// after a loss, it admits none.
pub fn admit_frame() -> bool {
    admission().as_ref().is_some_and(Admission::admits)
}

/// These are the numbers the exports below return besides 0, which means done.
/// They are the C ABI's, and an embedder's header states them as they are here
/// (@xpute/gpu ios/CXputeGpu/xpute_gpu.h).
///
/// `xpute_gpu_open` returns this when no device opens as the program asks.
pub const NO_DEVICE: i32 = -1;
/// `xpute_gpu_surface_bind` returns this before `xpute_gpu_open` is first
/// called.
pub const NOT_OPEN: i32 = -2;
/// `xpute_gpu_surface_bind` returns this when the system makes no surface of
/// the window.
pub const NO_SURFACE: i32 = -3;
/// `xpute_gpu_surface_bind` returns this when a newer handle has bound the slot
/// since.
pub const SLOT_TAKEN: i32 = -4;

/// The path a C string names, or none for a null pointer or a path that is no
/// UTF-8.
///
/// # Safety
/// `at` is null or a NUL-terminated string.
pub unsafe fn path(at: *const c_char) -> Option<PathBuf> {
    if at.is_null() {
        return None;
    }
    // SAFETY: the caller's NUL-terminated string.
    let s = unsafe { CStr::from_ptr(at) };
    s.to_str().ok().map(PathBuf::from)
}

/// Says what refused the device, the way a stop is said (xpute-runtime
/// native `say`).
pub fn say_refused(e: &OpenError) {
    xpute_runtime::native::say(&format!("the device did not open: {e:?}"));
}

/// Opens a device as `ask` says (`open`), on the instance the first call
/// makes, keeping its compiled pipelines in `cache_dir`, the directory the
/// platform keeps an app's caches in, where one is given (`Cache`). A device
/// already open is kept. A new device binds every window bound, in the order
/// bound, and the windows stay held until their unbind: one whose slot a newer
/// handle holds stays unbound, as at a bind, and one the system makes no
/// surface of now is said, its surface drawn into textures never shown until
/// the next device. The lost runtime's surfaces went with it, each destroyed
/// at once as it dropped, before any is made here. The error is what refused
/// the device.
pub fn open(ask: &Ask, cfg: Config, cache_dir: Option<&Path>) -> Result<(), OpenError> {
    let mut guard = device();
    let d = guard.get_or_insert_with(|| Device {
        instance: super::instance(ask),
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        windows: Vec::new(),
        runtime: None,
    });
    if d.runtime.is_some() {
        return Ok(());
    }
    let Wgpu {
        adapter,
        device,
        queue,
    } = super::open(&d.instance, ask)?;
    let runtime = d.runtime.insert(GpuRuntime::new(
        adapter,
        device,
        queue,
        cfg,
        cache_dir,
        xpute_runtime::native::wake,
    ));
    *admission() = Some(runtime.admission());
    #[cfg(any(target_vendor = "apple", target_os = "android"))]
    for (handle, window) in &d.windows {
        // SAFETY: a window the runtime holds a reference on.
        match unsafe { d.instance.create_surface_unsafe(target(window.0)) } {
            Ok(surface) => {
                runtime.bind(*handle, surface);
            }
            Err(e) => xpute_runtime::native::say(&format!(
                "[gpu] no surface of surface {handle:#x}'s window on this device: {e}"
            )),
        }
    }
    WORK.get_or_init(|| {
        let (work, jobs) = channel::<Job>();
        std::thread::Builder::new()
            .name("xpute-gpu".into())
            .spawn(move || jobs.into_iter().for_each(|job| job()))
            .expect("the system starts a thread");
        work
    });
    Ok(())
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

/// Binds the core's surface `handle` to `window`, on the open device and on
/// every device opened after it. Returns 0 when bound, `NOT_OPEN` before
/// `open` is first called, `NO_SURFACE` when the system makes no surface of
/// the window, and `SLOT_TAKEN` when a newer handle has bound the slot since.
/// While no device is open the window is kept, and the next device binds it
/// (`open`). The runtime holds a reference of its own on a window bound until
/// it is unbound (`Window`).
///
/// # Safety
/// `window` is a live window of the platform's kind (`target`).
#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub unsafe fn bind(handle: u32, window: NonNull<c_void>) -> i32 {
    let mut guard = device();
    let Some(d) = guard.as_mut() else {
        return NOT_OPEN;
    };
    if let Some(h) = d.runtime.as_mut() {
        // SAFETY: the caller's live window, which `Window` holds from here.
        let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(target(window)) }) else {
            return NO_SURFACE;
        };
        if !h.bind(handle, surface) {
            return SLOT_TAKEN;
        }
    }
    d.windows.retain(|&(bound, _)| bound != handle);
    // SAFETY: the caller's live window.
    d.windows.push((handle, unsafe { Window::of(window) }));
    0
}

/// Lets `handle`'s window go: its surface, if `handle` is still the one bound
/// in its slot (`GpuRuntime::unbind`), and then the runtime's reference on the
/// window `handle` bound, whatever the slot holds now.
pub fn unbind(handle: u32) {
    let mut guard = device();
    let Some(d) = guard.as_mut() else {
        return;
    };
    if let Some(h) = d.runtime.as_mut() {
        h.unbind(handle);
    }
    #[cfg(any(target_vendor = "apple", target_os = "android"))]
    d.windows.retain(|&(bound, _)| bound != handle);
}

/// Exports the GPU's runtime, for a core booted by xpute-runtime's
/// `native_core!` or `native_abi!`, under xpute's names:
///
/// - `xpute_gpu_open(const char *cache_dir) -> int32_t` opens a device
///   (`open`), keeping its compiled pipelines in `cache_dir`, the directory the
///   platform keeps the app's caches in, or nowhere when it is null. It
///   returns 0 when a device is open, newly or already, and `NO_DEVICE` when
///   none opens as asked, after saying what refused it (`say_refused`). After
///   a loss it opens a new device.
/// - `xpute_gpu_lost_on(void (*lost)(void))` makes `lost` be told of each
///   device lost, once a loss (`lost_on`); null sets none.
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`
///   binds the core's surface `handle` to `window` (`bind`), and returns
///   `NO_SURFACE` for a null window. It exists on an Apple platform and on
///   Android, whose windows `target` takes.
/// - `xpute_gpu_surface_unbind(uint32_t handle)` lets the window go
///   (`unbind`).
///
/// On Android the same calls are exported to JNI for the Kotlin class
/// `xpute.Gpu` (`jni`): `open(cacheDir: String?): Boolean`,
/// `surfaceBind(handle: Int, surface: Surface): Int` and
/// `surfaceUnbind(handle: Int)`, and a loss calls its static `onLost()`, which
/// the first `open` makes the function told (`jni::init`). A completion asks
/// for a turn (xpute-runtime native `wake`).
///
/// `ask` and `cfg` are the program's choices for the device (`Ask`, `Config`).
/// The program reaches the runtime for its own calls through `with_runtime`.
#[macro_export]
macro_rules! native_abi {
    ($ask:expr, $cfg:expr $(,)?) => {
        mod xpute_gpu_native {
            #[allow(unused_imports)]
            use super::*;

            pub fn open(cache_dir: Option<&::std::path::Path>) -> i32 {
                match $crate::runtime::native::open(&$ask, $cfg, cache_dir) {
                    Ok(()) => 0,
                    Err(e) => {
                        $crate::runtime::native::say_refused(&e);
                        $crate::runtime::native::NO_DEVICE
                    }
                }
            }

            /// # Safety
            /// The contract is the same as for `xpute_gpu_surface_bind`.
            #[cfg(any(target_vendor = "apple", target_os = "android"))]
            pub unsafe fn bind(handle: u32, window: *mut ::core::ffi::c_void) -> i32 {
                let Some(window) = ::core::ptr::NonNull::new(window) else {
                    return $crate::runtime::native::NO_SURFACE;
                };
                // SAFETY: the caller's live window.
                unsafe { $crate::runtime::native::bind(handle, window) }
            }

            pub fn unbind(handle: u32) {
                $crate::runtime::native::unbind(handle);
            }
        }

        /// # Safety
        /// `cache_dir` is null or a NUL-terminated path.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_gpu_open(cache_dir: *const ::core::ffi::c_char) -> i32 {
            // SAFETY: the caller's path, or none.
            let cache_dir = unsafe { $crate::runtime::native::path(cache_dir) };
            xpute_gpu_native::open(cache_dir.as_deref())
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_lost_on(lost: Option<$crate::runtime::native::Lost>) {
            $crate::runtime::native::lost_on(lost);
        }

        /// # Safety
        /// `window` is null or a live window of the platform's kind.
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        #[no_mangle]
        pub unsafe extern "C" fn xpute_gpu_surface_bind(
            handle: u32,
            window: *mut ::core::ffi::c_void,
        ) -> i32 {
            // SAFETY: the caller's window, or none.
            unsafe { xpute_gpu_native::bind(handle, window) }
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_surface_unbind(handle: u32) {
            xpute_gpu_native::unbind(handle);
        }

        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_open(
            env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            this: ::xpute_runtime::jni::sys::jobject,
            cache_dir: ::xpute_runtime::jni::sys::jstring,
        ) -> ::xpute_runtime::jni::sys::jboolean {
            // SAFETY: the environment, the `xpute.Gpu` and the string JNI
            // called with.
            let cache_dir = unsafe {
                $crate::runtime::jni::init(env, this);
                $crate::runtime::jni::path(env, cache_dir)
            };
            ::xpute_runtime::jni::sys::jboolean::from(
                xpute_gpu_native::open(cache_dir.as_deref()) == 0,
            )
        }

        /// # Safety
        /// Only JNI calls this, with a live `android.view.Surface`.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_surfaceBind(
            env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            _this: ::xpute_runtime::jni::sys::jobject,
            handle: ::xpute_runtime::jni::sys::jint,
            surface: ::xpute_runtime::jni::sys::jobject,
        ) -> ::xpute_runtime::jni::sys::jint {
            // SAFETY: the environment and surface JNI called with.
            unsafe {
                $crate::runtime::jni::bind(env, handle as u32, surface, xpute_gpu_native::bind)
            }
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Gpu_surfaceUnbind(
            _env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            _this: ::xpute_runtime::jni::sys::jobject,
            handle: ::xpute_runtime::jni::sys::jint,
        ) {
            xpute_gpu_native::unbind(handle as u32);
        }
    };
}

/// This defines a core of `manifest` (its `MANIFEST`) linked into this process
/// with the GPU's runtime beside it. This crate's `core!` expands to it off
/// WebAssembly. It is xpute-runtime's
/// `native_core!` with this runtime as its device: `poll` runs before each
/// turn, `publish` runs the stream at the manifest's `stream_at` as the core
/// publishes it. It is also `native_abi!`, with
/// the device `ask` declares (`Ask::of`) and `numbers` made a
/// `Config` by the platform's defaults (`config`). The program's crate depends
/// on xpute-core and xpute-runtime besides this crate.
#[macro_export]
macro_rules! native_core {
    ($manifest:expr, $fixed_bytes:expr, $entry:expr, $numbers:expr, $ask:expr $(,)?) => {
        ::xpute_runtime::native_core!(
            $manifest,
            $fixed_bytes,
            $entry,
            device: ::xpute_runtime::native::Device {
                poll: $crate::runtime::native::poll,
                publish: |core| $crate::runtime::native::publish(core, $manifest.stream_at()),
            },
            slow_ms: $numbers.slow_ms
        );

        $crate::native_abi!(
            $crate::runtime::Ask::of($ask),
            $crate::runtime::native::config($numbers)
        );
    };
}
