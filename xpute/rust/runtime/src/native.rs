// xpute-runtime/native.rs

//! The runtime for a core linked into its own process: it reserves the core
//! memory (`reserve`), runs the core on the core's own thread, where its
//! `Global`s are sound, as a vCPU that runs while it has work and sleeps
//! until it is kicked, at the moment the program's policy gives for the
//! time it asked for (`thread`), hands each
//! call the core makes to the thread that answers it (`answer_with`,
//! `notify_on`), and keeps the core's blocks in its memory (`Alloc`). One
//! core a process, since its base is one, so the C ABI is xpute's own names
//! with no program's prefix (`native_abi!`).

mod alloc;
mod reserve;
mod say;
mod thread;

use core::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Instant;

use xpute_core::mem::heap::HeapStat;
use xpute_core::mem::section::PAGE_BYTES;

pub use alloc::Alloc;
pub use say::{say, stop_hook};
pub use thread::{
    answer, answer_with, kick, kick_deadline, notify_on, pause, resume, ring, turns, Answer,
    Notify, Vcpu,
};
pub use xpute_core::entry::{run, BootInfo, Entry};
pub use xpute_core::machine::Machine;
pub use xpute_core::stop::PanicHook;

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
    pub now: fn() -> f64,
    /// `BootInfo::stop_hook`.
    pub stop_hook: Option<PanicHook>,
    /// `BootInfo::admit`.
    pub admit_frame: Option<fn() -> bool>,
    /// `BootInfo::publish`.
    pub publish: Option<fn()>,
    /// `Vcpu::ready`: whether a turn may begin (`ready`, for a core with
    /// the machine's rings).
    pub ready: fn() -> bool,
}

/// A device beside the core, such as its GPU's runtime, on the core's
/// thread, each handed the core: `poll` writes what it has completed into
/// the memory before each turn, and `publish` runs what the core has just
/// published to it, as the core rings it (`BootInfo::publish`). Its
/// admission is what the core is handed as `BootInfo::admit`.
#[derive(Clone, Copy)]
pub struct Device {
    pub poll: fn(Core),
    pub publish: fn(Core),
    pub admit_frame: Option<fn() -> bool>,
}

impl Device {
    /// No device: nothing polled or run, and every frame admitted.
    pub const NONE: Device = Device {
        poll: |_| {},
        publish: |_| {},
        admit_frame: None,
    };
}

/// `Vcpu::ready` for a core of a machine: before the first turn has opened
/// its rings, or while the completion ring has room for what one turn posts
/// (xpute-core ipc/door.rs `has_room`).
pub fn ready() -> bool {
    !xpute_core::machine::booted() || xpute_core::machine::door().has_room()
}

/// The only way to call into the booted core.
#[derive(Clone, Copy)]
pub struct Core {
    _booted: (),
}

/// Reserves the memory and starts the core's thread, marked as the core's
/// with its base set before this returns, which loops on `turn` and
/// `cfg.ready` (`Vcpu`). Once a process.
pub fn boot(cfg: &Config, turn: fn(deadline_ms: f64) -> f64) -> Core {
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
    assert!(reserve::core().is_none(), "a core is booted once a process");
    let core = reserve::map(cfg.mem_bytes as usize, cfg.heap_at as usize);
    let vcpu = Vcpu {
        turn,
        ready: cfg.ready,
    };
    thread::start(cfg.thread_name, cfg.stack_bytes, vcpu, move || {
        xpute_core::global::mark_thread();
        // SAFETY: the region was reserved for the process's life.
        unsafe { xpute_core::mem::set(core.at, core.bytes) };
    });
    Core { _booted: () }
}

impl Core {
    /// `f` on the core's thread between turns, the caller waiting, as QEMU's
    /// `run_on_cpu` runs work on a vCPU's thread, and answering on its own
    /// thread each call `f` makes (`answer_with`), a turn's too where the
    /// caller is the thread that rings; none where the core has stopped, in
    /// this call or before it. Refused on the core's thread, inside
    /// `while_idle` and inside an answer, where it would wait for itself.
    pub fn run_on<R: Send>(self, f: impl FnOnce() -> R + Send) -> Option<R> {
        thread::run(f)
    }

    /// `f` on this thread while the core's runs nothing. Refused where `run_on` is.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// Read through raw pointers, never as one reference over the whole: the
    /// runtime's own blocks lie in it too, and the core's thread runs beside
    /// the reader.
    pub fn mem(self) -> (NonNull<u8>, usize) {
        let c = reserve::core().expect("a Core is booted");
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
            now: cfg.now,
            call: thread::call,
            heap_stat: alloc::stat,
            stop_hook: cfg.stop_hook,
            admit_frame: cfg.admit_frame,
            publish: cfg.publish,
        }
    }
}

/// The process's clock in milliseconds, from its first reading.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// A turn of a core of `machine`: on the first, the front its runtime laid
/// held to the machine and opened (`machine::boot`), then `entry`'s.
pub fn turn(machine: &Machine, deadline_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !xpute_core::machine::booted() {
        xpute_core::machine::boot(machine);
    }
    entry(deadline_ms, boot)
}

/// Boots the core with `cfg` on the first call, its thread looping on
/// `entry` (`Entry`), and exports it under xpute's names
/// (`rust/runtime/include/xpute.h`): `xpute_run(double deadline_ms) -> double`, a
/// turn waited for (`ring`); `xpute_kick(double deadline_ms)`, a turn wanted
/// (`kick_deadline`); `xpute_mem(uint32_t *bytes) -> uint8_t *`, `bytes` null
/// or written with the memory's size; `xpute_call_on(int32_t
/// (*answer)(uint32_t))` (`answer_with`); `xpute_notify_on(void (*f)(void))`
/// (`notify_on`); `xpute_answer(void)` (`answer`); and
/// `xpute_turns(double *turn_ms) -> double` (`turns`). `core()` is the
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

            pub fn core() -> $crate::native::Core {
                static CORE: ::std::sync::OnceLock<$crate::native::Core> =
                    ::std::sync::OnceLock::new();
                *CORE.get_or_init(|| $crate::native::boot(cfg(), xpute_vcpu_turn))
            }

            fn boot_info() -> $crate::native::BootInfo {
                core().boot_info(cfg())
            }

            fn xpute_vcpu_turn(deadline_ms: f64) -> f64 {
                $crate::native::run(deadline_ms, boot_info, ENTRY)
            }

            pub fn run(deadline_ms: f64) -> f64 {
                core();
                $crate::native::ring(deadline_ms).unwrap_or(f64::NAN)
            }

            pub fn kick(deadline_ms: f64) {
                core();
                $crate::native::kick_deadline(deadline_ms);
            }

            pub fn mem(bytes: Option<&mut u32>) -> *mut u8 {
                let (at, n) = core().mem();
                if let Some(bytes) = bytes {
                    *bytes = u32::try_from(n).expect("the memory is at most 4 GiB");
                }
                at.as_ptr()
            }
        }

        pub use xpute_native::core;

        #[no_mangle]
        pub extern "C" fn xpute_run(deadline_ms: f64) -> f64 {
            xpute_native::run(deadline_ms)
        }

        #[no_mangle]
        pub extern "C" fn xpute_kick(deadline_ms: f64) {
            xpute_native::kick(deadline_ms)
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
        pub extern "C" fn xpute_call_on(answer: Option<$crate::native::Answer>) {
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
        pub unsafe extern "C" fn xpute_turns(turn_ms: *mut f64) -> f64 {
            let (wake, ms) = $crate::native::turns();
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

/// A core of `machine` (its `MACHINE`) linked into this process, its fixed
/// ranges `fixed_bytes` long, whose turns `entry` runs (`Entry`) once the
/// first has opened the front its runtime laid (`turn`): the native form
/// xpute-core's `core!` expands to off WebAssembly. It installs `Alloc` as
/// the global allocator and boots the core through `native_abi!`, with the
/// smaller of the machine's sizes, its own ranges past the machine's front,
/// the heap past the fixed ranges, the machine's thread stack, xpute's stop
/// hook (`stop_hook`) and a turn begun only where its rings have room
/// (`ready`). Given `device`, each turn runs after its poll, the core rings
/// it as it publishes, and is handed its admission (`Device`); xpute-gpu's
/// `native_core!` gives its runtime's.
///
/// On Android it also exports the C ABI's calls to JNI under the Kotlin class
/// `xpute.Core` (`jni`): `run(deadlineMs: Double): Double`,
/// `kick(deadlineMs: Double)`, `pause()`, `resume()`, `answer()`,
/// `turns(): DoubleArray` (the wake,
/// then the longest turn) and `mem(): ByteBuffer`, calling back its static
/// `call(at: Int): Int` for each call and `onNotify()` for what
/// `xpute_notify_on`'s function is told.
#[macro_export]
macro_rules! native_core {
    ($machine:expr, $fixed_bytes:expr, $entry:expr, device: $device:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);

        #[global_allocator]
        static XPUTE_ALLOC: $crate::native::Alloc = $crate::native::Alloc;

        mod xpute_native_core {
            #[allow(unused_imports)]
            use super::*;

            const ENTRY: $crate::native::Entry = $entry;
            pub(super) const DEVICE: $crate::native::Device = $device;

            pub(super) fn xpute_turn(deadline_ms: f64, boot: fn() -> $crate::native::BootInfo) -> f64 {
                (DEVICE.poll)(core());
                $crate::native::turn(&$machine, deadline_ms, boot, ENTRY)
            }

            pub(super) fn publish() {
                (DEVICE.publish)(core());
            }
        }

        $crate::native_abi!(
            $crate::native::Config {
                mem_bytes: $machine.min_mem_bytes(),
                data_end: $machine.global_base(),
                heap_at: $machine.heap_at($machine.global_base(), $fixed_bytes),
                stack_bytes: $machine.thread_stack_bytes as usize,
                thread_name: "xpute-core",
                now: $crate::native::now,
                stop_hook: Some($crate::native::stop_hook),
                admit_frame: xpute_native_core::DEVICE.admit_frame,
                publish: Some(xpute_native_core::publish),
                ready: $crate::native::ready,
            },
            xpute_native_core::xpute_turn
        );

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_run(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            deadline_ms: $crate::jni::sys::jdouble,
        ) -> $crate::jni::sys::jdouble {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_run(deadline_ms)
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_kick(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            deadline_ms: $crate::jni::sys::jdouble,
        ) {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_kick(deadline_ms);
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_pause(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_pause();
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_resume(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_resume();
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_answer(
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
        unsafe extern "system" fn Java_xpute_Core_turns(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::turns(env) }
        }

        /// # Safety
        /// Called by JNI.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_mem(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment and object JNI called with.
            unsafe {
                $crate::jni::init(env, this);
                $crate::jni::mem(env, core())
            }
        }
    };
    ($machine:expr, $fixed_bytes:expr, $entry:expr $(,)?) => {
        $crate::native_core!($machine, $fixed_bytes, $entry, device: $crate::native::Device::NONE);
    };
}
