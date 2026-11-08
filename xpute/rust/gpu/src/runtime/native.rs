// xpute-gpu/runtime/native.rs

//! The GPU's runtime for a core linked into its own process (xpute-runtime
//! native), and its C ABI under xpute's names (`native_abi!`). On Android the
//! Kotlin class `xpute.Gpu` reaches it too (`jni`). The device is opened once
//! and held. A platform's window is bound to a core's surface and let go. The
//! stream runs as the core publishes it, and a completion kicks the core.
//!
//! The runtime is touched only on the core's thread: through `Core::run_on`,
//! and through the core's own calls into `publish` and `admit_frame`. That is
//! what makes its `Global` sound. There is one runtime a process, as there is
//! one core, so its exports carry no program's prefix.

#[cfg(target_os = "android")]
pub mod jni;

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;

use xpute_core::global::Global;
use xpute_runtime::mem::Memory;
use xpute_runtime::native::Core;

use super::{Ask, Config, ConfigureAt, GpuRuntime, OpenError, Opened};
use crate::Numbers;

struct Device {
    /// What each window's surface is made with.
    instance: wgpu::Instance,
    runtime: GpuRuntime,
}

static DEVICE: Global<Option<Device>> = Global::new();

/// The core the device was opened for. `configure` holds it idle while a
/// surface is configured off its thread.
static CORE: OnceLock<Core> = OnceLock::new();

/// Where a native core's surfaces are configured: on an Apple platform, on
/// the main thread while the core is idle; elsewhere, on the core's thread,
/// which runs the stream.
#[cfg(target_vendor = "apple")]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Here;

/// The surface format the runtime prefers: BGRA on an Apple platform, and
/// RGBA elsewhere, since Android's Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The runtime's config for a native core: the program's `numbers`, the
/// platform's preferred format, where it configures a surface
/// (`CONFIGURE_AT`), and `Fifo`, which shows each frame in turn at the
/// display's pace and is the one present mode every platform has.
pub const fn config(numbers: Numbers) -> Config {
    Config {
        preferred_format: PREFERRED_FORMAT,
        configure_at: CONFIGURE_AT,
        frame_latency: numbers.frame_latency,
        upload_chunk_bytes: numbers.upload_chunk_bytes,
        present_mode: wgpu::PresentMode::Fifo,
    }
}

fn device() -> Option<&'static mut Device> {
    DEVICE.get(|| None).as_mut()
}

/// The runtime once the device is open, for the program's own calls. Call it
/// on the core's thread only.
pub fn runtime() -> Option<&'static mut GpuRuntime> {
    device().map(|d| &mut d.runtime)
}

#[cfg(target_vendor = "apple")]
fn configure() {
    if let Some(core) = CORE.get() {
        core.while_idle(|| {
            if let Some(h) = runtime() {
                h.configure_surfaces();
            }
        });
    }
}

/// The core's memory as the runtime reads and writes it, on the core's
/// thread.
fn view(core: Core) -> Memory<'static> {
    let (base, bytes) = core.mem();
    // SAFETY: the memory, reserved for the process's life; on the core's
    // thread, before a turn or inside the core's publish, where the core
    // holds no reference into a range it has handed the runtime: the stream
    // and what its records name (xpute-core ipc/stream.rs R3), and a word
    // the runtime answers (R4).
    unsafe { Memory::from_raw(base, bytes) }
}

/// Writes into the core's memory what the device has answered since the last
/// turn. It runs before each turn, on the core's thread.
pub fn poll(core: Core) {
    if let Some(h) = runtime() {
        h.poll(&mut view(core));
    }
}

/// Runs the stream at `stream_at`, which the core has just published on its
/// thread by storing END with release (xpute-core `Form::publish`). The
/// records run on the device and the frames are shown, and END is stored
/// back to 0 with release once the last record has read what it names
/// (`GpuRuntime::run`). Before a device is open, the stream is handed back
/// unrun, so the core is never held and no record waits for a device that
/// opens later.
pub fn publish(core: Core, stream_at: u32) {
    match runtime() {
        Some(h) => {
            h.run(&mut view(core), stream_at);
            h.present();
        }
        None => super::run::release(&mut view(core), stream_at),
    }
}

/// The runtime's admission (`crate::admit`), handed to the core with its
/// boot (xpute-core `Form::admit_frame`). It admits a frame while the runtime
/// does (`GpuRuntime::admit_frame`), so taking a surface's texture never
/// blocks the core's thread, which a caller may be waiting on (xpute-runtime
/// native `ring`). Before the device opens, it admits every frame.
pub fn admit_frame() -> bool {
    runtime().is_none_or(|h| h.admit_frame())
}

/// The numbers the exports below return besides 0, which means done. They
/// are the C ABI's, and an embedder's header states them as they are here
/// (include/xpute_gpu.h).
///
/// `xpute_gpu_open`: no device opens as the program asks.
pub const NO_DEVICE: i32 = -1;
/// `xpute_gpu_surface_bind`: no device is open.
pub const NOT_OPEN: i32 = -2;
/// `xpute_gpu_surface_bind`: the system makes no surface of the window.
pub const NO_SURFACE: i32 = -3;
/// `xpute_gpu_surface_bind`: a newer handle has bound the slot since.
pub const SLOT_TAKEN: i32 = -4;

/// Says what refused the device, the way a stop is said (xpute-runtime
/// native `say`).
pub fn say_refused(e: &OpenError) {
    xpute_runtime::native::say(&format!("the device did not open: {e:?}"));
}

/// Opens the device as `ask` says (`open`), on `core`'s thread. A device
/// already open is kept. The error is what refused the device.
pub fn open(core: Core, ask: &Ask, cfg: Config) -> Result<(), OpenError> {
    let _ = CORE.set(core);
    let opened = core.run_on(move || {
        if device().is_some() {
            return Ok(());
        }
        let Opened {
            instance,
            adapter,
            device,
            queue,
        } = super::open(ask)?;
        let runtime = GpuRuntime::new(adapter, device, queue, cfg, xpute_runtime::native::kick);
        DEVICE.set(Some(Device { instance, runtime }));
        Ok(())
    });
    opened.unwrap_or(Err(OpenError::Terminated))
}

/// The target wgpu draws a platform's window through: on an Apple platform a
/// view's `CAMetalLayer`, on Android a view's native window.
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
/// slot since. The surface's target (`target`) is made on the core's thread,
/// where every surface is made. It is made there rather than handed there,
/// since it is raw pointers, which belong to no thread but the one that uses
/// them.
///
/// # Safety
/// `window` is a live window of the platform's kind (`target`), and its
/// owner keeps it until it has unbound the surface.
#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub unsafe fn bind(core: Core, handle: u32, window: NonNull<c_void>) -> i32 {
    let at = window.as_ptr().expose_provenance();
    let bound = core.run_on(move || {
        let Some(d) = device() else { return NOT_OPEN };
        let Some(window) = NonNull::new(core::ptr::with_exposed_provenance_mut(at)) else {
            return NO_SURFACE;
        };
        // SAFETY: the caller's window, kept until the surface is unbound.
        let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(target(window)) }) else {
            return NO_SURFACE;
        };
        if !d.runtime.bind(handle, surface) {
            return SLOT_TAKEN;
        }
        0
    });
    bound.unwrap_or(NOT_OPEN)
}

/// Lets `handle`'s window go, if `handle` is still the one bound in its slot
/// (`GpuRuntime::unbind`). A core that has stopped runs nothing more, so its
/// thread keeps what it held, the window too, for as long as the process
/// lives.
pub fn unbind(core: Core, handle: u32) {
    let _ = core.run_on(move || {
        if let Some(h) = runtime() {
            h.unbind(handle);
        }
    });
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
/// `surfaceUnbind(handle: Int)`. A completion kicks the core (xpute-runtime
/// native `kick`).
///
/// `core` gives the core (xpute-runtime `native_abi!`'s `core`); `ask` and
/// `cfg` are the program's choices for the device (`Ask`, `Config`). The
/// program reaches the runtime for its own calls through `runtime()`.
#[macro_export]
macro_rules! native_abi {
    ($core:expr, $ask:expr, $cfg:expr $(,)?) => {
        mod xpute_gpu_native {
            #[allow(unused_imports)]
            use super::*;

            pub fn open() -> i32 {
                match $crate::runtime::native::open(($core)(), &$ask, $cfg) {
                    Ok(()) => 0,
                    Err(e) => {
                        $crate::runtime::native::say_refused(&e);
                        $crate::runtime::native::NO_DEVICE
                    }
                }
            }

            /// # Safety
            /// The same as for `xpute_gpu_surface_bind`.
            #[cfg(any(target_vendor = "apple", target_os = "android"))]
            pub unsafe fn bind(handle: u32, window: *mut ::core::ffi::c_void) -> i32 {
                let Some(window) = ::core::ptr::NonNull::new(window) else {
                    return $crate::runtime::native::NO_SURFACE;
                };
                // SAFETY: the caller's window, kept until it is unbound.
                unsafe { $crate::runtime::native::bind(($core)(), handle, window) }
            }

            pub fn unbind(handle: u32) {
                $crate::runtime::native::unbind(($core)(), handle);
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
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_open(
            _env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            _this: ::xpute_runtime::jni::sys::jobject,
        ) -> ::xpute_runtime::jni::sys::jboolean {
            ::xpute_runtime::jni::sys::jboolean::from(xpute_gpu_native::open() == 0)
        }

        /// # Safety
        /// Called by JNI, with a live `android.view.Surface`, which its view
        /// keeps until it has unbound it.
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
                $crate::runtime::native::jni::bind(
                    env,
                    handle as u32,
                    surface,
                    xpute_gpu_native::bind,
                )
            }
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Gpu_surfaceUnbind(
            _env: *mut ::xpute_runtime::jni::sys::JNIEnv,
            _this: ::xpute_runtime::jni::sys::jobject,
            handle: ::xpute_runtime::jni::sys::jint,
        ) {
            $crate::runtime::native::jni::unbind(handle as u32, xpute_gpu_native::unbind);
        }
    };
}

/// A core of `machine` (its `MACHINE`) linked into this process with the
/// GPU's runtime beside it. xpute-core's `core!` expands to it off
/// WebAssembly when it is given the GPU's numbers. It is xpute-runtime's
/// `native_core!` with this runtime as its device: `poll` runs before each
/// turn, `publish` runs the stream at the machine's `gpu_at` as the core
/// rings, and `admit_frame` is the core's admission. It is also
/// `native_abi!`, with the device the machine's GPU declares (`Ask::of`) and
/// `numbers` made a `Config` by the platform's defaults (`config`). The
/// program's crate depends on xpute-core and xpute-runtime besides this
/// crate.
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
            core,
            $crate::runtime::Ask::of(&xpute_gpu_socket::GPU),
            $crate::runtime::native::config($numbers)
        );
    };
}
