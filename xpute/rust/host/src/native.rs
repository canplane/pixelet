// xpute-host/native.rs

//! The host for a guest linked into its own process: it reserves the core
//! memory (`reserve`), runs the guest on the guest's own thread, where its
//! `Global`s are sound, as a vCPU that runs while it has work and sleeps
//! until it is kicked, at the moment the program's policy gives for the
//! time it asked for (`thread`), hands each
//! hostcall the guest makes to the thread that answers it (`answer_with`,
//! `notify_on`), and keeps the guest's blocks in its memory (`Alloc`). One
//! guest a process, since its base is one, so the C ABI is xpute's own names
//! with no program's prefix (`native_abi!`).

mod alloc;
mod reserve;
mod say;
mod thread;

use core::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Instant;

use xpute_guest::mem::heap::HeapStat;
use xpute_guest::mem::section::PAGE_BYTES;

pub use alloc::Alloc;
pub use say::{say, stop_hook};
pub use thread::{
    ack, answer, answer_with, kick, kick_quota, notify_on, pause, resume, ring, Answer, Notify,
    Vcpu,
};
pub use xpute_guest::entry::{run, BootInfo, Entry};
pub use xpute_guest::machine::Machine;
pub use xpute_guest::stop::PanicHook;

pub struct Config {
    /// Reserved once, never grown.
    pub mem_bytes: u32,
    /// `BootInfo::data_end`: the library's statics are the process's, so the
    /// end of the front the program keeps at the memory's bottom, 0 for none.
    pub data_end: u32,
    /// A whole number of pages; the heap runs to the memory's end.
    pub heap_at: u32,
    pub stack_bytes: usize,
    pub thread_name: &'static str,
    pub clock: fn() -> f64,
    /// `BootInfo::stop_hook`.
    pub stop_hook: Option<PanicHook>,
    /// `BootInfo::admit`.
    pub admit: Option<fn() -> bool>,
    /// `BootInfo::publish`.
    pub publish: Option<fn()>,
    /// `Vcpu::ready`: whether a turn may begin (`ready`, for a guest with
    /// the machine's rings).
    pub ready: fn() -> bool,
}

/// A device beside the guest, such as its GPU's host, on the guest's
/// thread, each handed the guest: `poll` writes what it has completed into
/// the memory before each turn, and `publish` runs what the guest has just
/// published to it, as the guest rings it (`BootInfo::publish`). Its
/// admission is what the guest is handed as `BootInfo::admit`.
#[derive(Clone, Copy)]
pub struct Device {
    pub poll: fn(Guest),
    pub publish: fn(Guest),
    pub admit: Option<fn() -> bool>,
}

impl Device {
    /// No device: nothing polled or run, and every frame admitted.
    pub const NONE: Device = Device {
        poll: |_| {},
        publish: |_| {},
        admit: None,
    };
}

/// `Vcpu::ready` for a guest of a machine: before the first turn has opened
/// its rings, or while the completion ring has room for what one turn posts
/// (xpute-guest ipc/door.rs `has_room`).
pub fn ready() -> bool {
    !xpute_guest::machine::booted() || xpute_guest::machine::door().has_room()
}

/// The only way to call into the booted guest.
#[derive(Clone, Copy)]
pub struct Guest {
    _booted: (),
}

/// Reserves the memory and starts the guest's thread, marked as the guest's
/// with its base set before this returns, which loops on `turn` and
/// `cfg.ready` (`Vcpu`). Once a process.
pub fn boot(cfg: &Config, turn: fn(quota_ms: f64) -> f64) -> Guest {
    assert!(
        cfg.heap_at < cfg.mem_bytes,
        "the heap begins at {} of a {}-byte memory",
        cfg.heap_at,
        cfg.mem_bytes
    );
    assert!(
        cfg.heap_at.is_multiple_of(PAGE_BYTES),
        "the heap begins at {}, not on a {PAGE_BYTES}-byte page",
        cfg.heap_at
    );
    assert!(
        reserve::core().is_none(),
        "a guest is booted once a process"
    );
    let core = reserve::map(cfg.mem_bytes as usize, cfg.heap_at as usize);
    let vcpu = Vcpu {
        turn,
        ready: cfg.ready,
    };
    thread::start(cfg.thread_name, cfg.stack_bytes, vcpu, move || {
        xpute_guest::global::mark_thread();
        // SAFETY: the region was reserved for the process's life.
        unsafe { xpute_guest::mem::set(core.at, core.bytes) };
    });
    Guest { _booted: () }
}

impl Guest {
    /// `f` on the guest's thread between turns, the caller waiting, and
    /// answering on its own thread each hostcall `f` makes (`answer_with`),
    /// a turn's too where the caller is the thread that rings. Refused on the
    /// guest's thread, inside `while_idle` and inside an answer, where it
    /// would wait for itself. For what a stopped guest is never asked; else
    /// `turn`.
    pub fn run<R: Send>(self, f: impl FnOnce() -> R + Send) -> R {
        thread::run(f).expect("the guest has stopped")
    }

    /// `run`, none where the guest has stopped, in this call or before it.
    pub fn turn<R: Send>(self, f: impl FnOnce() -> R + Send) -> Option<R> {
        thread::run(f)
    }

    /// `f` on this thread while the guest's runs nothing. Refused where `run` is.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// Read through raw pointers, never as one reference over the whole: the
    /// host's own blocks lie in it too, and the guest's thread runs beside
    /// the reader.
    pub fn mem(self) -> (NonNull<u8>, usize) {
        let c = reserve::core().expect("a Guest is booted");
        (
            NonNull::new(core::ptr::with_exposed_provenance_mut(c.at))
                .expect("a mapping is not at 0"),
            c.bytes,
        )
    }

    pub fn heap_stat(self) -> HeapStat {
        alloc::stat()
    }

    pub fn boot_info(self, cfg: &Config) -> BootInfo {
        BootInfo {
            given: u32::try_from(self.mem().1).expect("the memory is at most 4 GiB"),
            data_end: cfg.data_end,
            clock: cfg.clock,
            hostcall: thread::hostcall,
            heap_stat: alloc::stat,
            stop_hook: cfg.stop_hook,
            admit: cfg.admit,
            publish: cfg.publish,
        }
    }
}

/// The process's clock in milliseconds, from its first reading.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// A turn of a guest of `machine`: on the first, the front its host laid
/// held to the machine and opened (`machine::boot`), then `entry`'s.
pub fn turn(machine: &Machine, quota_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !xpute_guest::machine::booted() {
        xpute_guest::machine::boot(machine);
    }
    entry(quota_ms, boot)
}

/// Boots the guest with `cfg` on the first call, its thread looping on
/// `entry` (`Entry`), and exports it under xpute's names
/// (`rust/host/include/xpute.h`): `xpute_run(double quota_ms) -> double`, a
/// turn waited for (`ring`); `xpute_kick(double quota_ms)`, a turn wanted
/// (`kick_quota`); `xpute_mem(uint32_t *bytes) -> uint8_t *`, `bytes` null
/// or written with the memory's size; `xpute_hostcall_on(int32_t
/// (*answer)(uint32_t))` (`answer_with`); `xpute_notify_on(void (*f)(void))`
/// (`notify_on`); `xpute_answer(void)` (`answer`); and
/// `xpute_ack(double *turn_ms) -> double` (`ack`). `guest()` is the
/// program's for its own calls.
#[macro_export]
macro_rules! native_abi {
    ($cfg:expr, $entry:expr $(,)?) => {
        mod xpute_native {
            #[allow(unused_imports)]
            use super::*;

            const ENTRY: $crate::native::Entry = $entry;

            fn cfg() -> &'static $crate::native::Config {
                static CFG: ::std::sync::OnceLock<$crate::native::Config> =
                    ::std::sync::OnceLock::new();
                CFG.get_or_init(|| $cfg)
            }

            pub fn guest() -> $crate::native::Guest {
                static GUEST: ::std::sync::OnceLock<$crate::native::Guest> =
                    ::std::sync::OnceLock::new();
                *GUEST.get_or_init(|| $crate::native::boot(cfg(), xpute_vcpu_turn))
            }

            fn boot_info() -> $crate::native::BootInfo {
                guest().boot_info(cfg())
            }

            fn xpute_vcpu_turn(quota_ms: f64) -> f64 {
                $crate::native::run(quota_ms, boot_info, ENTRY)
            }

            pub fn run(quota_ms: f64) -> f64 {
                guest();
                $crate::native::ring(quota_ms).unwrap_or(f64::NAN)
            }

            pub fn kick(quota_ms: f64) {
                guest();
                $crate::native::kick_quota(quota_ms);
            }

            pub fn mem(bytes: Option<&mut u32>) -> *mut u8 {
                let (at, n) = guest().mem();
                if let Some(bytes) = bytes {
                    *bytes = u32::try_from(n).expect("the memory is at most 4 GiB");
                }
                at.as_ptr()
            }
        }

        pub use xpute_native::guest;

        #[no_mangle]
        pub extern "C" fn xpute_run(quota_ms: f64) -> f64 {
            xpute_native::run(quota_ms)
        }

        #[no_mangle]
        pub extern "C" fn xpute_kick(quota_ms: f64) {
            xpute_native::kick(quota_ms)
        }

        #[no_mangle]
        pub extern "C" fn xpute_pause() {
            $crate::native::pause();
        }

        #[no_mangle]
        pub extern "C" fn xpute_resume() {
            $crate::native::resume();
        }

        #[no_mangle]
        pub extern "C" fn xpute_hostcall_on(answer: Option<$crate::native::Answer>) {
            $crate::native::answer_with(answer);
        }

        #[no_mangle]
        pub extern "C" fn xpute_notify_on(notify: Option<$crate::native::Notify>) {
            $crate::native::notify_on(notify);
        }

        #[no_mangle]
        pub extern "C" fn xpute_answer() {
            $crate::native::answer();
        }

        /// # Safety
        /// `turn_ms` is null or points at an `f64` the caller owns.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_ack(turn_ms: *mut f64) -> f64 {
            let (wake, ms) = $crate::native::ack();
            // SAFETY: the caller's word, or none.
            if let Some(out) = unsafe { turn_ms.as_mut() } {
                *out = ms;
            }
            wake
        }

        /// # Safety
        /// `bytes` is null or points at a `u32` the caller owns.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_mem(bytes: *mut u32) -> *mut u8 {
            // SAFETY: the caller's word, or none.
            xpute_native::mem(unsafe { bytes.as_mut() })
        }
    };
}

/// A guest of `machine` (its `MACHINE`) linked into this process, its fixed
/// ranges `fixed_bytes` long, whose turns `entry` runs (`Entry`) once the
/// first has opened the front its host laid (`turn`): the native form
/// xpute-guest's `guest!` expands to off WebAssembly. It installs `Alloc` as
/// the global allocator and boots the guest through `native_abi!`, with the
/// smaller of the machine's sizes, its own ranges past the machine's front,
/// the heap past the fixed ranges, the machine's thread stack, xpute's stop
/// hook (`stop_hook`) and a turn begun only where its rings have room
/// (`ready`). Given `device`, each turn runs after its poll, the guest rings
/// it as it publishes, and is handed its admission (`Device`); xpute-gpu's
/// `native_guest!` gives its host's.
///
/// On Android it also exports the C ABI's calls to JNI under the Kotlin class
/// `xpute.Guest` (`jni`): `run(quotaMs: Double): Double`,
/// `kick(quotaMs: Double)`, `pause()`, `resume()`, `answer()`,
/// `ack(): DoubleArray` (the wake,
/// then the longest turn) and `mem(): ByteBuffer`, calling back its static
/// `hostcall(at: Int): Int` for each hostcall and `onNotify()` for what
/// `xpute_notify_on`'s function is told.
#[macro_export]
macro_rules! native_guest {
    ($machine:expr, $fixed_bytes:expr, $entry:expr, device: $device:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);

        #[global_allocator]
        static XPUTE_ALLOC: $crate::native::Alloc = $crate::native::Alloc;

        mod xpute_native_guest {
            #[allow(unused_imports)]
            use super::*;

            const ENTRY: $crate::native::Entry = $entry;
            pub(super) const DEVICE: $crate::native::Device = $device;

            pub(super) fn xpute_turn(quota_ms: f64, boot: fn() -> $crate::native::BootInfo) -> f64 {
                (DEVICE.poll)(guest());
                $crate::native::turn(&$machine, quota_ms, boot, ENTRY)
            }

            pub(super) fn publish() {
                (DEVICE.publish)(guest());
            }
        }

        $crate::native_abi!(
            $crate::native::Config {
                mem_bytes: $machine.min_mem_bytes(),
                data_end: $machine.global_base(),
                heap_at: $machine.heap_at($machine.global_base(), $fixed_bytes),
                stack_bytes: $machine.thread_stack_bytes as usize,
                thread_name: "xpute-guest",
                clock: $crate::native::now,
                stop_hook: Some($crate::native::stop_hook),
                admit: xpute_native_guest::DEVICE.admit,
                publish: Some(xpute_native_guest::publish),
                ready: $crate::native::ready,
            },
            xpute_native_guest::xpute_turn
        );

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_run(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            quota_ms: $crate::jni::sys::jdouble,
        ) -> $crate::jni::sys::jdouble {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_run(quota_ms)
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_kick(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            quota_ms: $crate::jni::sys::jdouble,
        ) {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_kick(quota_ms);
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_pause(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_pause();
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_resume(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_resume();
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_answer(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_answer();
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_ack(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::ack(env) }
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Guest_mem(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment and object JNI called with.
            unsafe {
                $crate::jni::init(env, this);
                $crate::jni::mem(env, guest())
            }
        }
    };
    ($machine:expr, $fixed_bytes:expr, $entry:expr $(,)?) => {
        $crate::native_guest!($machine, $fixed_bytes, $entry, device: $crate::native::Device::NONE);
    };
}
