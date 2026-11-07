// xpute-gpu/runtime/native.rs

//! The GPU's runtime beside a core linked into its own process (xpute-runtime
//! native), and its C ABI under xpute's names (`native_abi!`), on Android
//! under the Kotlin class `xpute.Gpu` as well (`jni`): the device opened once
//! and held, a platform's window bound to a core's surface and let go, the
//! stream run as the core publishes it, and a completion that kicks the
//! core. The runtime is touched on the core's thread alone (`Core::run`, and
//! the core's own calls into `publish` and `admit`), where its `Global` is
//! sound, so there is one a process, as there is one core, and no program's
//! prefix.

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

/// The core the device was opened for, idle while a surface is configured
/// off its thread (`configure`).
static CORE: OnceLock<Core> = OnceLock::new();

/// Where a native core's surfaces are configured, for its `Config`: on an
/// Apple platform's main thread while the core is idle; elsewhere on the
/// core's thread, which runs the stream.
#[cfg(target_vendor = "apple")]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Here;

/// The format a surface is preferred in: BGRA on an Apple platform, RGBA
/// elsewhere, as Android's Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
pub const PREFERRED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The runtime's config beside a native core: the program's `numbers`, the
/// platform's preferred format and where it configures a surface
/// (`CONFIGURE_AT`), and each frame shown in turn at the display's pace
/// (`Fifo`), the one present mode every platform has.
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

/// The runtime once the device is open, for the program's own calls: on the
/// core's thread alone.
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

/// Before each turn, on the core's thread: what the device has answered
/// since the last, written into the memory before the turn reads it.
pub fn poll(core: Core) {
    if let Some(h) = runtime() {
        h.poll(&mut view(core));
    }
}

/// The core has just published the stream at `stream_at`, END stored with
/// release (xpute-core `Form::publish`), on its thread: its records run
/// on the device once END is loaded with acquire, and its frames shown, then
/// END stored back to 0 with release after the last record has read what it
/// names (`GpuRuntime::run`); before a device is open, the stream handed back
/// unrun, so the core is never held and no record waits for a device that
/// answers later.
pub fn publish(core: Core, stream_at: u32) {
    match runtime() {
        Some(h) => {
            h.run(&mut view(core), stream_at);
            h.present();
        }
        None => super::run::release(&mut view(core), stream_at),
    }
}

/// The runtime's admission (`crate::admit`), handed the core with its boot
/// (xpute-core `Form::admit_frame`): a frame while the runtime admits one
/// (`GpuRuntime::admit_frame`), so taking a surface's texture never blocks the
/// core's thread, which a caller may be waiting on (xpute-runtime native
/// `ring`); every frame before the device opens.
pub fn admit_frame() -> bool {
    runtime().is_none_or(|h| h.admit_frame())
}

/// What the exports below answer besides 0, done: the C ABI's numbers, which
/// an embedder's header states as they are here.
///
/// `xpute_gpu_open`: no device opened as the program asks.
pub const NO_DEVICE: i32 = -1;
/// `xpute_gpu_surface_bind`: no device is open.
pub const NOT_OPEN: i32 = -2;
/// `xpute_gpu_surface_bind`: the system would not make a surface of the window.
pub const NO_SURFACE: i32 = -3;
/// `xpute_gpu_surface_bind`: a newer handle has bound the slot since.
pub const SLOT_TAKEN: i32 = -4;

/// What refused the device, said as a stop is (xpute-runtime native `say`).
pub fn say_refused(e: &OpenError) {
    xpute_runtime::native::say(&format!("the device did not open: {e:?}"));
}

/// Opens the device as `ask` says (`open`), once, on `core`'s thread:
/// open, or what refused it.
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

/// What a platform's window is made for wgpu to draw through: on an Apple
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

/// Binds the core's surface `handle` to `window`, made a surface's target
/// (`target`) on the core's thread, where every surface is made: 0 when
/// bound, `NOT_OPEN` when no device is open, `NO_SURFACE` when the system
/// would not make a surface of it, `SLOT_TAKEN` when a newer handle has bound
/// its slot since. The target is made there, not handed there, since it is
/// raw pointers and no thread's but the one that uses it.
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

/// Lets `handle`'s window go, if it is still the one bound under its slot
/// (`GpuRuntime::unbind`). A stopped core's thread keeps what it held, the window
/// among it, as the process goes on.
pub fn unbind(core: Core, handle: u32) {
    let _ = core.run_on(move || {
        if let Some(h) = runtime() {
            h.unbind(handle);
        }
    });
}

/// Exports the GPU's runtime beside a core booted by xpute-runtime's
/// `native_core!` or `native_abi!`, under xpute's names:
///
/// - `xpute_gpu_open(void) -> int32_t`: opens the device, 0 when it is open,
///   `NO_DEVICE` when none opened as asked, after what refused it is said
///   (`say_refused`).
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`:
///   binds the core's surface `handle` to `window` (`bind`), `NO_SURFACE`
///   for a null one; on an Apple platform and Android, whose windows
///   `target` takes.
/// - `xpute_gpu_surface_unbind(uint32_t handle)`: lets it go (`unbind`).
///
/// On Android it also exports them to JNI under the Kotlin class `xpute.Gpu`
/// (`jni`): `open(): Boolean`, `surfaceBind(handle: Int, surface: Surface): Int`
/// and `surfaceUnbind(handle: Int)`. A completion kicks the core (xpute-runtime
/// native `kick`).
///
/// `core` is the core's (`native_abi!`'s `core`), and `ask` and `cfg` the
/// program's for the device (`Ask`, `Config`). `runtime()` is the program's for
/// its own calls.
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
            /// As `xpute_gpu_surface_bind`'s.
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
/// GPU's runtime beside it, the form xpute-core's `core!` expands to off
/// WebAssembly when it is given the GPU's numbers: xpute-runtime's
/// `native_core!` with this runtime its device (`poll` before each turn,
/// `publish` over the stream at the machine's `gpu_at` as the core rings,
/// and its admission, `admit`), and `native_abi!` with the device the
/// machine's GPU device asks for (`Ask::of`) and `numbers` made a `Config`
/// with the platform's defaults (`config`). The program's crate depends on
/// xpute-core and xpute-runtime besides this crate.
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
