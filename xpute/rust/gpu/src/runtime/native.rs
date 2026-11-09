// xpute-gpu/runtime/native.rs

//! This is the GPU's runtime for a core linked into its own process
//! (xpute-runtime native), and its C ABI under xpute's names (`native_abi!`).
//! On Android the Kotlin class `xpute.Gpu` reaches it too (`jni`). The device
//! is opened once and held. A platform's window is bound to a core's surface
//! and let go. A published stream runs beside the core, and a completion asks
//! for a turn.
//!
//! No turn waits on the device. `publish` hands its work over and returns, and
//! `poll` skips a turn the device is busy for. An async pipeline is compiled
//! apart from the stream, and a sync one holds it (xpute-core ipc/stream.rs):
//! the core goes on turning and records nothing until END comes back. A frame's
//! present waits behind the stream, and holds only the streams published after
//! it. There is one runtime a
//! process, as there is one core, so its exports carry no program's prefix.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_runtime::mem::Memory;
use xpute_runtime::native::Core;

use super::admit::Admission;
use super::{Ask, Config, ConfigureAt, GpuRuntime, OpenError, Opened};
use crate::CoreConfig;

struct Device {
    /// Each window's surface is made with this.
    instance: wgpu::Instance,
    runtime: GpuRuntime,
}

/// Whoever uses the runtime holds this, so a surface is bound or configured
/// only while no stream runs.
static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

type Job = Box<dyn FnOnce() + Send>;

/// The device's queue of work, in the order the core asked for it. It exists
/// once a device has opened.
static WORK: OnceLock<Sender<Job>> = OnceLock::new();

/// The open device's admission, read as the core asks (`admit_frame`).
static ADMISSION: OnceLock<Admission> = OnceLock::new();

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
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn device() -> MutexGuard<'static, Option<Device>> {
    DEVICE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f` on the runtime once the device is open, for the program's own
/// calls. It waits for a stream that is running.
pub fn with_runtime<R>(f: impl FnOnce(&mut GpuRuntime) -> R) -> Option<R> {
    device().as_mut().map(|d| f(&mut d.runtime))
}

/// Runs `f` where the stream runs, after all the work the core has asked for
/// so far, while the caller waits. An error scope the program pushes there
/// catches the errors of the records run after it. Before a device is open it
/// answers none.
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
    // records name until END comes back (xpute-core ipc/stream.rs R3) and the
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
        d.runtime.poll(&mut view(core));
    }
}

/// Takes the stream at `stream_at`, which the core has just published by
/// storing END with release (xpute-core `Form::publish`). The records run on
/// the device and END is stored back to 0 with release, as a page's runtime
/// does, and only then are the frames shown, since showing one can wait for
/// the frame before it. Before a device is open, the stream is handed back
/// unrun, so the core is never held and no record waits for a device that
/// opens later.
pub fn publish(core: Core, stream_at: u32) {
    let Some(work) = WORK.get() else {
        super::run::release(&mut view(core), stream_at);
        return;
    };
    let job: Job = Box::new(move || {
        let mut mem = view(core);
        let answered = with_runtime(|h| {
            h.run(&mut mem, stream_at);
            h.present();
            h.poll(&mut mem)
        });
        if answered == Some(true) {
            xpute_runtime::native::wake();
        }
    });
    let _ = work.send(job);
}

/// This is the runtime's admission (`crate::admit`), handed to the core with
/// its boot (xpute-core `Form::admit_frame`). It admits a frame while fewer
/// than the frame latency are unfinished. Before the device opens, it admits
/// every frame.
pub fn admit_frame() -> bool {
    ADMISSION.get().is_none_or(Admission::admits)
}

/// These are the numbers the exports below return besides 0, which means done.
/// They are the C ABI's, and an embedder's header states them as they are here
/// (include/xpute_gpu.h).
///
/// `xpute_gpu_open` returns this when no device opens as the program asks.
pub const NO_DEVICE: i32 = -1;
/// `xpute_gpu_surface_bind` returns this when no device is open.
pub const NOT_OPEN: i32 = -2;
/// `xpute_gpu_surface_bind` returns this when the system makes no surface of
/// the window.
pub const NO_SURFACE: i32 = -3;
/// `xpute_gpu_surface_bind` returns this when a newer handle has bound the slot
/// since.
pub const SLOT_TAKEN: i32 = -4;

/// Says what refused the device, the way a stop is said (xpute-runtime
/// native `say`).
pub fn say_refused(e: &OpenError) {
    xpute_runtime::native::say(&format!("the device did not open: {e:?}"));
}

/// Opens the device as `ask` says (`open`). A device already open is kept. The
/// error is what refused the device.
pub fn open(ask: &Ask, cfg: Config) -> Result<(), OpenError> {
    let mut guard = device();
    if guard.is_some() {
        return Ok(());
    }
    let Opened {
        instance,
        adapter,
        device,
        queue,
    } = super::open(ask)?;
    let runtime = GpuRuntime::new(adapter, device, queue, cfg, xpute_runtime::native::wake);
    let _ = ADMISSION.set(runtime.admission());
    *guard = Some(Device { instance, runtime });
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

/// Binds the core's surface `handle` to `window`. Returns 0 when bound,
/// `NOT_OPEN` when no device is open, `NO_SURFACE` when the system makes no
/// surface of the window, and `SLOT_TAKEN` when a newer handle has bound the
/// slot since.
///
/// # Safety
/// `window` is a live window of the platform's kind (`target`), and its
/// owner keeps it until it has unbound the surface.
#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub unsafe fn bind(handle: u32, window: NonNull<c_void>) -> i32 {
    let mut guard = device();
    let Some(d) = guard.as_mut() else {
        return NOT_OPEN;
    };
    // SAFETY: the caller's window, kept until the surface is unbound.
    let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(target(window)) }) else {
        return NO_SURFACE;
    };
    if !d.runtime.bind(handle, surface) {
        return SLOT_TAKEN;
    }
    0
}

/// Lets `handle`'s window go, if `handle` is still the one bound in its slot
/// (`GpuRuntime::unbind`).
pub fn unbind(handle: u32) {
    with_runtime(|h| h.unbind(handle));
}

/// Exports the GPU's runtime, for a core booted by xpute-runtime's
/// `native_core!` or `native_abi!`, under xpute's names:
///
/// - `xpute_gpu_open(void) -> int32_t` opens the device. It returns 0 when
///   the device is open, and `NO_DEVICE` when none opens as asked, after
///   saying what refused it (`say_refused`).
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`
///   binds the core's surface `handle` to `window` (`bind`), and returns
///   `NO_SURFACE` for a null window. It exists on an Apple platform and on
///   Android, whose windows `target` takes.
/// - `xpute_gpu_surface_unbind(uint32_t handle)` lets the window go
///   (`unbind`).
///
/// On Android the same calls are exported to JNI for the Kotlin class
/// `xpute.Gpu` (`jni`): `open(): Boolean`,
/// `surfaceBind(handle: Int, surface: Surface): Int` and
/// `surfaceUnbind(handle: Int)`. A completion asks for a turn (xpute-runtime
/// native `wake`).
///
/// `ask` and `cfg` are the program's choices for the device (`Ask`, `Config`).
/// The program reaches the runtime for its own calls through `with_runtime`.
#[macro_export]
macro_rules! native_abi {
    ($ask:expr, $cfg:expr $(,)?) => {
        mod xpute_gpu_native {
            #[allow(unused_imports)]
            use super::*;

            pub fn open() -> i32 {
                match $crate::runtime::native::open(&$ask, $cfg) {
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
                // SAFETY: the caller's window, kept until it is unbound.
                unsafe { $crate::runtime::native::bind(handle, window) }
            }

            pub fn unbind(handle: u32) {
                $crate::runtime::native::unbind(handle);
            }
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_open() -> i32 {
            xpute_gpu_native::open()
        }

        /// # Safety
        /// `window` is null or a live window of the platform's kind, which
        /// its owner keeps until it has unbound the surface.
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
        /// Only JNI calls this.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_open(
            _env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            _this: ::xpute_runtime::jni::sys::jobject,
        ) -> ::xpute_runtime::jni::sys::jboolean {
            ::xpute_runtime::jni::sys::jboolean::from(xpute_gpu_native::open() == 0)
        }

        /// # Safety
        /// Only JNI calls this, with a live `android.view.Surface`, which its
        /// view keeps until it has unbound it.
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
            $crate::runtime::jni::unbind(handle as u32, xpute_gpu_native::unbind);
        }
    };
}

/// This defines a core of `machine` (its `MACHINE`) linked into this process
/// with the GPU's runtime beside it. xpute-core's `core!` expands to it off
/// WebAssembly when it is given the GPU's numbers. It is xpute-runtime's
/// `native_core!` with this runtime as its device: `poll` runs before each
/// turn, `publish` runs the stream at the machine's `gpu_at` as the core rings,
/// and `admit_frame` is the core's admission. It is also `native_abi!`, with
/// the device the machine's GPU declares (`Ask::of`) and `numbers` made a
/// `Config` by the platform's defaults (`config`). The program's crate depends
/// on xpute-core and xpute-runtime besides this crate.
#[macro_export]
macro_rules! native_core {
    ($machine:expr, $fixed_bytes:expr, $entry:expr, $numbers:expr $(,)?) => {
        ::xpute_runtime::native_core!(
            $machine,
            $fixed_bytes,
            $entry,
            device: ::xpute_runtime::native::Device {
                poll: $crate::runtime::native::poll,
                publish: |core| $crate::runtime::native::publish(core, $machine.gpu_at()),
                admit_frame: Some($crate::runtime::native::admit_frame),
            }
        );

        mod xpute_gpu_socket {
            #[allow(unused_imports)]
            use super::*;

            pub(super) const GPU: ::xpute_core::machine::Gpu = match $machine.gpu {
                Some(gpu) => gpu,
                None => panic!("a core with the GPU's runtime takes the GPU device"),
            };
        }

        $crate::native_abi!(
            $crate::runtime::Ask::of(&xpute_gpu_socket::GPU),
            $crate::runtime::native::config($numbers)
        );
    };
}
