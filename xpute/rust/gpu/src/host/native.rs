// xpute-gpu/host/native.rs

//! The GPU's host beside a guest linked into its own process (xpute-host
//! native), and its C ABI under xpute's names (`native_abi!`): the device
//! opened once and held, a platform's window bound to a guest's surface and
//! let go, and what a completion calls to ask a turn. The host is touched on
//! the guest's thread alone (`Guest::run`), where its `Global` is sound, so
//! there is one a process, as there is one guest, and no program's prefix.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use xpute_guest::global::Global;
use xpute_host::native::Guest;

use super::{Ask, ConfigureAt, GpuHost, Opened, Plan};

struct Device {
    /// What each window's surface is made with.
    instance: wgpu::Instance,
    host: GpuHost,
}

static DEVICE: Global<Option<Device>> = Global::new();

/// The guest the device was opened for, idle while a surface is configured
/// off its thread (`configure`).
static GUEST: OnceLock<Guest> = OnceLock::new();

/// What `wake_on` was given, as an address; 0 for none.
static WAKE: AtomicUsize = AtomicUsize::new(0);

/// Where a native guest's surfaces are configured, for its `Plan`: on an
/// Apple platform's main thread while the guest is idle, since Core
/// Animation sets a layer's properties there alone; elsewhere on the guest's
/// thread, which runs the stream.
#[cfg(target_vendor = "apple")]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
pub const CONFIGURE_AT: ConfigureAt = ConfigureAt::Here;

fn device() -> Option<&'static mut Device> {
    DEVICE.get(|| None).as_mut()
}

/// The host once the device is open, for the program's own calls around a
/// turn: on the guest's thread alone.
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

/// `f` is what a completion calls to ask a turn, from whichever thread
/// learned of it (`GpuHost::new`'s `wake`).
pub fn wake_on(f: extern "C" fn()) {
    WAKE.store(f as usize, Ordering::Release);
}

fn wake() {
    let f = WAKE.load(Ordering::Acquire);
    if f != 0 {
        // SAFETY: an `extern "C" fn()` `wake_on` was given, stored as its address.
        let f: extern "C" fn() = unsafe { core::mem::transmute(f) };
        f();
    }
}

/// Opens the device as `ask` says (`open`), once, on `guest`'s thread:
/// whether it is open.
pub fn open(guest: Guest, ask: &Ask, plan: Plan) -> bool {
    let _ = GUEST.set(guest);
    guest.run(move || {
        if device().is_some() {
            return true;
        }
        let Ok(Opened {
            instance,
            adapter,
            device,
            queue,
        }) = super::open(ask)
        else {
            return false;
        };
        let host = GpuHost::new(adapter, device, queue, plan, wake);
        DEVICE.set(Some(Device { instance, host }));
        true
    })
}

/// Binds the guest's surface `handle` to `window`, which `target` makes a
/// surface's target of on the guest's thread, where every surface is made:
/// 0 when bound, -2 when no device is open, -3 when the system would not make
/// a surface of it, -4 when a newer handle has bound its slot since. The
/// target is made there, not handed there, since it is raw pointers and no
/// thread's but the one that uses it.
///
/// # Safety
/// `window` is live and is what `target` takes, and its owner keeps it until
/// it has unbound the surface.
pub unsafe fn bind(
    guest: Guest,
    handle: u32,
    window: NonNull<c_void>,
    target: fn(NonNull<c_void>) -> wgpu::SurfaceTargetUnsafe,
) -> i32 {
    let at = window.as_ptr().expose_provenance();
    guest.run(move || {
        let Some(d) = device() else { return -2 };
        let Some(window) = NonNull::new(core::ptr::with_exposed_provenance_mut(at)) else {
            return -3;
        };
        // SAFETY: the caller's window, kept until the surface is unbound.
        let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(target(window)) }) else {
            return -3;
        };
        if !d.host.bind(handle, surface) {
            return -4;
        }
        0
    })
}

/// Lets `handle`'s window go, if it is still the one bound under its slot
/// (`GpuHost::unbind`).
pub fn unbind(guest: Guest, handle: u32) {
    guest.run(move || {
        if let Some(h) = host() {
            h.unbind(handle);
        }
    });
}

/// Exports the GPU's host beside a guest booted by xpute-host's
/// `native_abi!`, under xpute's names:
///
/// - `xpute_gpu_open(void) -> int32_t`: opens the device, 0 when it is open,
///   -1 when the system has none.
/// - `xpute_gpu_wake_on(void (*f)(void))`: `f` asks the platform's loop for a
///   turn, from whichever thread learned of a completion.
/// - `xpute_gpu_surface_bind(uint32_t handle, void *window) -> int32_t`:
///   binds the guest's surface `handle` to `window` (`bind`), -3 for a null
///   one.
/// - `xpute_gpu_surface_unbind(uint32_t handle)`: lets it go (`unbind`).
///
/// `guest` is the guest's (`native_abi!`'s `guest`), `ask` and `plan` the
/// program's for the device (`Ask`, `Plan`), and `target` makes a surface's
/// target of a window as the platform hands it over
/// (`fn(NonNull<c_void>) -> wgpu::SurfaceTargetUnsafe`). `host()` is the
/// program's for its own calls around a turn.
#[macro_export]
macro_rules! native_abi {
    ($guest:expr, $ask:expr, $plan:expr, $target:expr $(,)?) => {
        mod xpute_gpu_native {
            #[allow(unused_imports)]
            use super::*;

            pub fn open() -> i32 {
                if $crate::host::native::open(($guest)(), &$ask, $plan) {
                    0
                } else {
                    -1
                }
            }

            /// # Safety
            /// As `xpute_gpu_surface_bind`'s.
            pub unsafe fn bind(handle: u32, window: *mut ::core::ffi::c_void) -> i32 {
                let Some(window) = ::core::ptr::NonNull::new(window) else {
                    return -3;
                };
                // SAFETY: the caller's window, kept until it is unbound.
                unsafe { $crate::host::native::bind(($guest)(), handle, window, $target) }
            }

            pub fn unbind(handle: u32) {
                $crate::host::native::unbind(($guest)(), handle);
            }
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_open() -> i32 {
            xpute_gpu_native::open()
        }

        #[no_mangle]
        pub extern "C" fn xpute_gpu_wake_on(f: extern "C" fn()) {
            $crate::host::native::wake_on(f);
        }

        /// # Safety
        /// `window` is null or a live window of the kind the program's
        /// `target` takes, which its owner keeps until it has unbound the
        /// surface.
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
    };
}
