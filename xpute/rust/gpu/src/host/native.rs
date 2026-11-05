// xpute-gpu/host/native.rs

//! The GPU's host beside a guest linked into its own process (xpute-host
//! native), and its C ABI under xpute's names (`native_abi!`), on Android
//! under the Kotlin class `xpute.Gpu` as well (`jni`): the device opened once
//! and held, a platform's window bound to a guest's surface and let go, the
//! stream run as the guest publishes it, and a completion that kicks the
//! guest. The host is touched on the guest's thread alone (`Guest::run`, and
//! the guest's own calls into `publish` and `admit`), where its `Global` is
//! sound, so there is one a process, as there is one guest, and no program's
//! prefix.

#[cfg(target_os = "android")]
pub mod jni;

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;

use xpute_guest::global::Global;
use xpute_host::mem::Memory;
use xpute_host::native::Guest;

use super::{Ask, Config, ConfigureAt, GpuHost, OpenError, Opened};
use crate::Numbers;

struct Device {
    /// What each window's surface is made with.
    instance: wgpu::Instance,
    host: GpuHost,
}

static DEVICE: Global<Option<Device>> = Global::new();

/// The guest the device was opened for, idle while a surface is configured
/// off its thread (`configure`).
static GUEST: OnceLock<Guest> = OnceLock::new();

/// Where a native guest's surfaces are configured, for its `Config`: on an
/// Apple platform's main thread while the guest is idle; elsewhere on the
/// guest's thread, which runs the stream.
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

/// The host's config beside a native guest: the program's `numbers`, the
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

/// The host once the device is open, for the program's own calls: on the
/// guest's thread alone.
pub fn host() -> Option<&'static mut GpuHost> {
    device().map(|d| &mut d.host)
}

#[cfg(target_vendor = "apple")]
fn configure() {
    if let Some(guest) = GUEST.get() {
        guest.while_idle(|| {
            if let Some(h) = host() {
                h.configure_surfaces();
            }
        });
    }
}

/// The guest's memory as the host reads and writes it, on the guest's
/// thread.
fn view(guest: Guest) -> Memory<'static> {
    let (base, bytes) = guest.mem();
    // SAFETY: the memory, reserved for the process's life; on the guest's
    // thread, before a turn or inside the guest's publish, where the guest
    // holds no reference into a range it has handed the host: the stream
    // and what its records name (xpute-guest ipc/stream.rs R3), and a word
    // the host answers (R4).
    unsafe { Memory::from_raw(base, bytes) }
}

/// Before each turn, on the guest's thread: what the device has answered
/// since the last, written into the memory before the turn reads it.
pub fn poll(guest: Guest) {
    if let Some(h) = host() {
        h.poll(&mut view(guest));
    }
}

/// The guest has just published the stream at `stream_at`, END stored with
/// release (xpute-guest `BootInfo::publish`), on its thread: its records run
/// on the device once END is loaded with acquire, and its frames shown, then
/// END stored back to 0 with release after the last record has read what it
/// names (`GpuHost::run`); before a device is open, the stream handed back
/// unrun, so the guest is never held and no record waits for a device that
/// answers later.
pub fn publish(guest: Guest, stream_at: u32) {
    match host() {
        Some(h) => {
            h.run(&mut view(guest), stream_at);
            h.present();
        }
        None => super::run::release(&mut view(guest), stream_at),
    }
}

/// The host's admission (`crate::admit`), handed the guest with its boot
/// (xpute-guest `BootInfo::admit`): a frame while the host admits one
/// (`GpuHost::admit`), so taking a surface's texture never blocks the
/// guest's thread, which a caller may be waiting on (xpute-host native
/// `ring`); every frame before the device opens.
pub fn admit() -> bool {
    host().is_none_or(|h| h.admit())
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

/// What refused the device, said as a stop is (xpute-host native `say`).
pub fn say_refused(e: &OpenError) {
    xpute_host::native::say(&format!("the device did not open: {e:?}"));
}

/// Opens the device as `ask` says (`open`), once, on `guest`'s thread:
/// open, or what refused it.
pub fn open(guest: Guest, ask: &Ask, cfg: Config) -> Result<(), OpenError> {
    let _ = GUEST.set(guest);
    let opened = guest.turn(move || {
        if device().is_some() {
            return Ok(());
        }
        let Opened {
            instance,
            adapter,
            device,
            queue,
        } = super::open(ask)?;
        let host = GpuHost::new(adapter, device, queue, cfg, xpute_host::native::kick);
        DEVICE.set(Some(Device { instance, host }));
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

/// Binds the guest's surface `handle` to `window`, made a surface's target
/// (`target`) on the guest's thread, where every surface is made: 0 when
/// bound, `NOT_OPEN` when no device is open, `NO_SURFACE` when the system
/// would not make a surface of it, `SLOT_TAKEN` when a newer handle has bound
/// its slot since. The target is made there, not handed there, since it is
/// raw pointers and no thread's but the one that uses it.
///
/// # Safety
/// `window` is a live window of the platform's kind (`target`), and its
/// owner keeps it until it has unbound the surface.
#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub unsafe fn bind(guest: Guest, handle: u32, window: NonNull<c_void>) -> i32 {
    let at = window.as_ptr().expose_provenance();
    let bound = guest.turn(move || {
        let Some(d) = device() else { return NOT_OPEN };
        let Some(window) = NonNull::new(core::ptr::with_exposed_provenance_mut(at)) else {
            return NO_SURFACE;
        };
        // SAFETY: the caller's window, kept until the surface is unbound.
        let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(target(window)) }) else {
            return NO_SURFACE;
        };
        if !d.host.bind(handle, surface) {
            return SLOT_TAKEN;
        }
        0
    });
    bound.unwrap_or(NOT_OPEN)
}

/// Lets `handle`'s window go, if it is still the one bound under its slot
/// (`GpuHost::unbind`). A stopped guest's thread keeps what it held, the window
/// among it, as the process goes on.
pub fn unbind(guest: Guest, handle: u32) {
    let _ = guest.turn(move || {
        if let Some(h) = host() {
            h.unbind(handle);
        }
    });
}

/// Exports the GPU's host beside a guest booted by xpute-host's
/// `native_guest!` or `native_abi!`, under xpute's names:
///
/// - `xpute_gpu_open(void) -> int32_t`: opens the device, 0 when it is open,
///   `NO_DEVICE` when none opened as asked, after what refused it is said
///   (`say_refused`).
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`:
///   binds the guest's surface `handle` to `window` (`bind`), `NO_SURFACE`
///   for a null one; on an Apple platform and Android, whose windows
///   `target` takes.
/// - `xpute_gpu_surface_unbind(uint32_t handle)`: lets it go (`unbind`).
///
/// On Android it also exports them to JNI under the Kotlin class `xpute.Gpu`
/// (`jni`): `open(): Boolean`, `surfaceBind(handle: Int, surface: Surface): Int`
/// and `surfaceUnbind(handle: Int)`. A completion kicks the guest (xpute-host
/// native `kick`).
///
/// `guest` is the guest's (`native_abi!`'s `guest`), and `ask` and `cfg` the
/// program's for the device (`Ask`, `Config`). `host()` is the program's for
/// its own calls.
#[macro_export]
macro_rules! native_abi {
    ($guest:expr, $ask:expr, $cfg:expr $(,)?) => {
        mod xpute_gpu_native {
            #[allow(unused_imports)]
            use super::*;

            pub fn open() -> i32 {
                match $crate::host::native::open(($guest)(), &$ask, $cfg) {
                    Ok(()) => 0,
                    Err(e) => {
                        $crate::host::native::say_refused(&e);
                        $crate::host::native::NO_DEVICE
                    }
                }
            }

            /// # Safety
            /// As `xpute_gpu_surface_bind`'s.
            #[cfg(any(target_vendor = "apple", target_os = "android"))]
            pub unsafe fn bind(handle: u32, window: *mut ::core::ffi::c_void) -> i32 {
                let Some(window) = ::core::ptr::NonNull::new(window) else {
                    return $crate::host::native::NO_SURFACE;
                };
                // SAFETY: the caller's window, kept until it is unbound.
                unsafe { $crate::host::native::bind(($guest)(), handle, window) }
            }

            pub fn unbind(handle: u32) {
                $crate::host::native::unbind(($guest)(), handle);
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
            _env: *mut ::xpute_host::jni::sys::JNIEnv,
            _this: ::xpute_host::jni::sys::jobject,
        ) -> ::xpute_host::jni::sys::jboolean {
            ::xpute_host::jni::sys::jboolean::from(xpute_gpu_native::open() == 0)
        }

        /// # Safety
        /// Called by JNI, with a live `android.view.Surface`, which its view
        /// keeps until it has unbound it.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_surfaceBind(
            env: *mut ::xpute_host::jni::sys::JNIEnv,
            _this: ::xpute_host::jni::sys::jobject,
            handle: ::xpute_host::jni::sys::jint,
            surface: ::xpute_host::jni::sys::jobject,
        ) -> ::xpute_host::jni::sys::jint {
            // SAFETY: the environment and surface JNI called with.
            unsafe {
                $crate::host::native::jni::bind(env, handle as u32, surface, xpute_gpu_native::bind)
            }
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Gpu_surfaceUnbind(
            _env: *mut ::xpute_host::jni::sys::JNIEnv,
            _this: ::xpute_host::jni::sys::jobject,
            handle: ::xpute_host::jni::sys::jint,
        ) {
            $crate::host::native::jni::unbind(handle as u32, xpute_gpu_native::unbind);
        }
    };
}

/// A guest of `machine` (its `MACHINE`) linked into this process with the
/// GPU's host beside it, the form xpute-guest's `guest!` expands to off
/// WebAssembly when it is given the GPU's numbers: xpute-host's
/// `native_guest!` with this host its device (`poll` before each turn,
/// `publish` over the stream at the machine's `gpu_at` as the guest rings,
/// and its admission, `admit`), and `native_abi!` with the device the
/// machine's GPU socket asks for (`Ask::of`) and `numbers` made a `Config`
/// with the platform's defaults (`config`). The program's crate depends on
/// xpute-guest and xpute-host besides this crate.
#[macro_export]
macro_rules! native_guest {
    ($machine:expr, $fixed_bytes:expr, $entry:expr, $numbers:expr $(,)?) => {
        ::xpute_host::native_guest!(
            $machine,
            $fixed_bytes,
            $entry,
            device: ::xpute_host::native::Device {
                poll: $crate::host::native::poll,
                publish: |guest| $crate::host::native::publish(guest, $machine.gpu_at()),
                admit: Some($crate::host::native::admit),
            }
        );

        mod xpute_gpu_socket {
            #[allow(unused_imports)]
            use super::*;

            pub(super) const GPU: ::xpute_guest::machine::Gpu = match $machine.gpu {
                Some(gpu) => gpu,
                None => panic!("a guest with the GPU's host takes the GPU socket"),
            };
        }

        $crate::native_abi!(
            guest,
            $crate::host::Ask::of(&xpute_gpu_socket::GPU),
            $crate::host::native::config($numbers)
        );
    };
}
