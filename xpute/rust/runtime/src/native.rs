// xpute-runtime/native.rs

//! This is the runtime for a core linked into the runtime's own process. It
//! reserves the core's memory (`reserve`) and runs the core on a thread of its
//! own, the only thread on which the core's `Global`s are sound. That thread
//! acts as a vCPU (`thread`): it runs a turn at a display frame when one is
//! wanted, and sleeps otherwise. Each call the core makes is handed to
//! the thread that answers it (`answer_with`, `notify_on`), and the blocks the
//! core allocates lie in its memory (`Alloc`).
//!
//! A process holds one core, because the core's memory base is one value for
//! the whole process. So the C ABI exports xpute's own names, with no prefix
//! of the program's (`native_abi!`).

mod alloc;
mod pacing;
mod reserve;
mod say;
mod serve;
mod thread;

use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};

use xpute_core::mem::heap::HeapStat;
use xpute_core::mem::section::PAGE_BYTES;

pub use alloc::Alloc;
pub use say::{say, stop_hook};
pub use serve::{attach, call, fs_closed, fs_opened, named_by, post, take, Named, STOPPED};
pub use thread::{
    answer, answer_with, input, notify_on, pace, pause, resume, ring, terminated, vsync, wake,
    Answer, Notify, Pace, Vcpu,
};
pub use xpute_core::entry::{run, BootInfo, Entry, Form};
pub use xpute_core::machine::Machine;
pub use xpute_core::stop::PanicHook;

pub struct Config {
    /// The size of the core's memory, reserved once and never grown.
    pub mem_bytes: u32,
    /// Where the heap begins, a whole number of pages into the memory. The
    /// heap runs to the memory's end.
    pub heap_at: u32,
    pub stack_bytes: usize,
    pub thread_name: &'static str,
    /// What the core is told about the form it was built in. A native
    /// library's statics live in the process, not in the core's memory, so
    /// its `data_end` is the end of the front the program lays at the bottom
    /// of the memory, or 0 when there is none.
    pub form: Form,
    /// Whether a turn may begin now (`Vcpu::ready`). A core whose machine
    /// has rings passes this module's `ready`.
    pub ready: fn() -> bool,
}

/// A device that runs beside the core, such as the GPU's runtime. Both
/// functions are handed the core. `poll` runs before each turn and has the
/// memory told what the device has completed. `publish` takes what the core
/// has just published to the device, as the core rings it (`Form::publish`). `admit_frame` is handed to the core as
/// `Form::admit_frame`.
#[derive(Clone, Copy)]
pub struct Device {
    pub poll: fn(Core),
    pub publish: fn(Core),
    pub admit_frame: Option<fn() -> bool>,
}

impl Device {
    pub const NONE: Device = Device {
        poll: |_| {},
        publish: |_| {},
        admit_frame: None,
    };
}

/// This is `Vcpu::ready` for a core with a machine. A turn may begin before the
/// first turn has opened the rings, and after that only while the completion
/// ring has room for all that one turn may post (xpute-core ipc/door.rs
/// `has_room`).
pub fn ready() -> bool {
    !xpute_core::machine::booted() || xpute_core::machine::door().has_room()
}

/// Proof that the core has booted; every call into the core goes through it.
#[derive(Clone, Copy)]
pub struct Core {
    _booted: (),
}

/// Reserves the memory and starts the core's thread, which loops on `turn` and
/// `cfg.ready` (`Vcpu`). Before this returns, the thread is marked as the
/// core's and the memory's base is set. A process boots once; a second boot
/// panics.
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
    /// Runs `f` on the core's thread between turns while the caller waits,
    /// as QEMU's `run_on_cpu` runs work on a vCPU's thread. Meanwhile the
    /// caller answers, on its own thread, each call `f` makes
    /// (`answer_with`); a caller that is the thread that rings answers the
    /// calls of turns too. Returns `None` when the core has stopped, during
    /// this call or before it. It panics on the core's thread, inside
    /// `while_idle` and inside an answer, where it would wait for itself.
    pub fn run_on<R: Send>(self, f: impl FnOnce() -> R + Send) -> Option<R> {
        thread::run(f)
    }

    /// Runs `f` on this thread while the core's thread runs nothing. It
    /// panics where `run_on` does.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// The memory's first byte and its size. Read it through raw pointers,
    /// never as one reference over the whole: the runtime's own blocks lie
    /// in it too, and the core's thread runs beside the reader.
    pub fn mem(self) -> (NonNull<u8>, usize) {
        let c = reserve::core().expect("a Core is booted");
        (
            NonNull::new(core::ptr::with_exposed_provenance_mut(c.at))
                .expect("a mapping is not at 0"),
            c.bytes,
        )
    }

    /// Loads the word at byte `at` of the memory with acquire, for a runtime
    /// whose language has no atomics of its own.
    pub fn load(self, at: u32) -> u32 {
        self.word(at).load(Ordering::Acquire)
    }

    /// Stores `word` at byte `at` of the memory with release, for the same
    /// runtime as `load`.
    pub fn store(self, at: u32, word: u32) {
        self.word(at).store(word, Ordering::Release);
    }

    fn word(self, at: u32) -> &'static AtomicU32 {
        let (mem, bytes) = self.mem();
        assert!(
            at.is_multiple_of(4) && at as usize + 4 <= bytes,
            "no word at {at} of a {bytes}-byte memory"
        );
        // SAFETY: an aligned word inside the memory, which lives as long as the
        // process, and which both ends touch only as an atomic.
        unsafe { AtomicU32::from_ptr(mem.as_ptr().add(at as usize).cast()) }
    }

    pub fn heap_stat(self) -> HeapStat {
        alloc::stat()
    }

    pub fn boot_info(self, cfg: &Config) -> BootInfo {
        BootInfo {
            mem_sz: u32::try_from(self.mem().1).expect("the memory is at most 4 GiB"),
            call: thread::call,
            heap_stat: alloc::stat,
            form: cfg.form,
        }
    }
}

/// The system's monotonic clock in milliseconds since boot: the clock a
/// native app's `performance.now()` counts on, C++'s `steady_clock`, so that the
/// core's times and the UI's lie on one line. That is `CLOCK_UPTIME_RAW` on
/// Apple's systems, which stops while the device sleeps, and `CLOCK_MONOTONIC`
/// elsewhere.
pub fn now() -> f64 {
    #[cfg(target_vendor = "apple")]
    const CLOCK: libc::clockid_t = libc::CLOCK_UPTIME_RAW;
    #[cfg(not(target_vendor = "apple"))]
    const CLOCK: libc::clockid_t = libc::CLOCK_MONOTONIC;
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid timespec to write, and CLOCK exists on every
    // system `native` builds for.
    unsafe { libc::clock_gettime(CLOCK, &raw mut t) };
    t.tv_sec as f64 * 1000.0 + t.tv_nsec as f64 / 1e6
}

/// Runs one turn of a core of `machine`. Before the first turn, the front the
/// runtime laid is checked against the machine and opened (`machine::boot`);
/// every turn then runs `entry`.
pub fn turn(machine: &Machine, deadline_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !xpute_core::machine::booted() {
        xpute_core::machine::boot(machine);
    }
    entry(deadline_ms, boot)
}

/// Exports the core under xpute's C names, which
/// `rust/runtime/include/xpute.h` declares and documents. The first call to
/// any of them boots the core with `cfg`, its thread looping on `entry`
/// (`Entry`). The generated `core()` is left to the program for its own
/// calls into the core.
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
        pub extern "C" fn xpute_wake() {
            $crate::native::wake()
        }

        #[no_mangle]
        pub extern "C" fn xpute_input() {
            $crate::native::input()
        }

        #[no_mangle]
        pub extern "C" fn xpute_vsync(age_ms: f64) {
            $crate::native::vsync(age_ms)
        }

        #[no_mangle]
        pub extern "C" fn xpute_load(at: u32) -> u32 {
            xpute_native::core().load(at)
        }

        #[no_mangle]
        pub extern "C" fn xpute_store(at: u32, word: u32) {
            xpute_native::core().store(at, word);
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

        #[no_mangle]
        pub extern "C" fn xpute_attach(sq_at: u32, cq_at: u32, fs: bool) {
            let (at, bytes) = xpute_native::core().mem();
            // SAFETY: the core's memory, reserved for the process's life, whose
            // rings the caller has laid.
            unsafe { $crate::native::attach(at.as_ptr() as usize, bytes, sq_at, cq_at, fs) };
        }

        #[no_mangle]
        pub extern "C" fn xpute_pace(
            pace_hz: f64,
            margin_share: f64,
            batch_frames: f64,
            settle_ms: f64,
        ) {
            $crate::native::pace($crate::native::Pace {
                pace_hz,
                margin_share,
                batch_frames,
                settle_ms,
            });
        }

        /// # Safety
        /// `pkt` points at `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_post(
            tag: u32,
            cmd: u32,
            flags: u32,
            result: i32,
            pkt: *const u8,
            len: usize,
        ) {
            let pkt = if len == 0 {
                &[][..]
            } else {
                // SAFETY: the caller's bytes.
                unsafe { core::slice::from_raw_parts(pkt, len) }
            };
            $crate::native::post(tag, cmd, flags, result, pkt);
        }

        /// # Safety
        /// `len` points at a word the caller owns. The bytes are the caller's
        /// until `xpute_taken_free`.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_take(len: *mut usize) -> *mut u8 {
            let bytes = Box::into_raw($crate::native::take().into_boxed_slice());
            // SAFETY: the caller's word.
            unsafe { *len = bytes.len() };
            bytes.cast()
        }

        /// # Safety
        /// `at` and `len` are what one `xpute_take` gave.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_taken_free(at: *mut u8, len: usize) {
            // SAFETY: the box `xpute_take` let go of.
            drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(at, len)) });
        }

        /// # Safety
        /// `bytes` points at `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_fs_opened(
            fd: u32,
            result: i32,
            detail: u32,
            bytes: *const u8,
            len: usize,
        ) {
            let file = if result < 0 || len == 0 {
                Vec::new()
            } else {
                // SAFETY: the caller's bytes.
                unsafe { core::slice::from_raw_parts(bytes, len) }.to_vec()
            };
            $crate::native::fs_opened(fd, result, detail, Some(file));
        }

        #[no_mangle]
        pub extern "C" fn xpute_fs_closed(fd: u32, result: i32) {
            $crate::native::fs_closed(fd, result);
        }

        /// # Safety
        /// `pkt` points at `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_call(cmd: u32, pkt: *const u8, len: usize) -> i32 {
            let pkt = if len == 0 {
                &[][..]
            } else {
                // SAFETY: the caller's bytes.
                unsafe { core::slice::from_raw_parts(pkt, len) }
            };
            $crate::native::call(cmd, pkt)
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

/// Links a core of `machine` (its `MACHINE`) into this process; this is the
/// native form that xpute-core's `core!` expands to outside WebAssembly.
/// `fixed_bytes` is the length of the core's fixed ranges, and `entry` runs
/// each turn (`Entry`) once the first turn has opened the front the runtime
/// laid (`turn`).
///
/// It installs `Alloc` as the global allocator and boots the core through
/// `native_abi!` with the smallest memory the machine allows, the core's own
/// ranges just past the machine's front, the heap past the fixed ranges, the
/// machine's thread stack, xpute's stop hook (`stop_hook`), and turns that
/// begin only while the rings have room (`ready`). Given a `device`, each
/// turn runs after the device's poll, the core rings the device as it
/// publishes, and the core is handed the device's admission (`Device`);
/// xpute-gpu's `native_core!` passes its runtime this way.
///
/// On Android it also exports the C ABI to JNI, as the `external` functions of
/// the Kotlin class `xpute.Core` (`jni`): `run(deadlineMs: Double): Double`,
/// `wake()`, `input()`, `vsync(ageMs: Double)`, `pause()`, `resume()`,
/// `answer()`, `attach(sqAt: Int, cqAt: Int, fs: Boolean)`, `pace(paceHz: Double,
/// marginShare: Double, batchFrames: Double, settleMs: Double)`, `post(tag: Int, cmd: Int, flags:
/// Int, result: Int, pkt: ByteArray)`, `take(): ByteArray`, `fsOpened(fd: Int,
/// result: Int, detail: Int, bytes: ByteArray)`, `fsClosed(fd: Int, result:
/// Int)`, `call(cmd: Int, pkt: ByteArray): Int`, `mem(): ByteBuffer`, `load(at: Double): Double` and
/// `store(at: Double, word: Double)`. It calls back the class's static
/// `call(at: Int): Int` for each of the core's calls, and its static
/// `onNotify()` for each piece of news `xpute_notify_on` describes.
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
                heap_at: $machine.heap_at($machine.global_base(), $fixed_bytes),
                stack_bytes: $machine.thread_stack_bytes as usize,
                thread_name: "xpute-core",
                ready: $crate::native::ready,
                form: $crate::native::Form {
                    data_end: $machine.global_base(),
                    now: $crate::native::now,
                    stop_hook: Some($crate::native::stop_hook),
                    admit_frame: xpute_native_core::DEVICE.admit_frame,
                    publish: Some(xpute_native_core::publish),
                },
            },
            xpute_native_core::xpute_turn
        );

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Core_wake(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_wake();
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Core_input(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_input();
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Core_vsync(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            age_ms: $crate::jni::sys::jdouble,
        ) {
            xpute_vsync(age_ms);
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_pause(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_pause();
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_resume(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            xpute_resume();
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
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
        /// Only JNI calls this, with the environment and object of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_attach(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            sq_at: $crate::jni::sys::jint,
            cq_at: $crate::jni::sys::jint,
            fs: $crate::jni::sys::jboolean,
        ) {
            // SAFETY: the environment and object JNI called with.
            unsafe { $crate::jni::init(env, this) };
            xpute_attach(sq_at as u32, cq_at as u32, fs != 0);
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Core_pace(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            pace_hz: $crate::jni::sys::jdouble,
            margin_share: $crate::jni::sys::jdouble,
            batch_frames: $crate::jni::sys::jdouble,
            settle_ms: $crate::jni::sys::jdouble,
        ) {
            xpute_pace(pace_hz, margin_share, batch_frames, settle_ms);
        }

        /// # Safety
        /// Only JNI calls this, with the environment and arguments of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_post(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            tag: $crate::jni::sys::jint,
            cmd: $crate::jni::sys::jint,
            flags: $crate::jni::sys::jint,
            result: $crate::jni::sys::jint,
            pkt: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: the environment and array JNI called with.
            let pkt = unsafe { $crate::jni::bytes(env, pkt) };
            $crate::native::post(tag as u32, cmd as u32, flags as u32, result, &pkt);
        }

        /// # Safety
        /// Only JNI calls this, with the environment of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_take(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jbyteArray {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::byte_array(env, &$crate::native::take()) }
        }

        /// # Safety
        /// Only JNI calls this, with the environment and arguments of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_fsOpened(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            fd: $crate::jni::sys::jint,
            result: $crate::jni::sys::jint,
            detail: $crate::jni::sys::jint,
            bytes: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: the environment and array JNI called with.
            let bytes = unsafe { $crate::jni::bytes(env, bytes) };
            $crate::native::fs_opened(fd as u32, result, detail as u32, Some(bytes));
        }

        #[cfg(target_os = "android")]
        #[no_mangle]
        extern "system" fn Java_xpute_Core_fsClosed(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            fd: $crate::jni::sys::jint,
            result: $crate::jni::sys::jint,
        ) {
            xpute_fs_closed(fd as u32, result);
        }

        /// # Safety
        /// Only JNI calls this, with the environment and arguments of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_call(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
            cmd: $crate::jni::sys::jint,
            pkt: $crate::jni::sys::jbyteArray,
        ) -> $crate::jni::sys::jint {
            // SAFETY: the environment, object and array JNI called with.
            let pkt = unsafe {
                $crate::jni::init(env, this);
                $crate::jni::bytes(env, pkt)
            };
            $crate::native::call(cmd as u32, &pkt)
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_load(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            at: $crate::jni::sys::jdouble,
        ) -> $crate::jni::sys::jdouble {
            f64::from(xpute_load(at as u32))
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
        #[cfg(target_os = "android")]
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Core_store(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            at: $crate::jni::sys::jdouble,
            word: $crate::jni::sys::jdouble,
        ) {
            xpute_store(at as u32, word as u32);
        }

        /// # Safety
        /// Only JNI calls this, with the environment and object of its call.
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
