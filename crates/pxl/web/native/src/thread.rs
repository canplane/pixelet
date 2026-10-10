// pxl-native/thread.rs

//! The kernel's own thread. The app calls in from whichever thread its shell
//! runs on — React Native's JavaScript thread, whose stack is 1 MiB — and a
//! turn that draws through wgpu can take more than that
//! (decision/20260926-a-device-s-kernel-on-a-thread-of-its-own.md). So every
//! call that reaches the kernel is handed to this thread, with a stack the
//! kernel sizes, and the caller waits for it: one of the two runs at a time,
//! and the kernel is single-threaded on this thread rather than the
//! caller's.
//!
//! The handoff is made once and used for every call: a turn a frame, and a
//! call that made a job and a channel for its answer each time allocated
//! twice a frame for nothing. Callers are more than one thread — the
//! JavaScript's rings the doorbell, the main thread binds a view's layer —
//! so they take the handoff in turn.

use std::cell::Cell;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The kernel's stack: the main thread's default on Linux and macOS. Not
/// derived — how deep wgpu's internals go has no bound this crate can state.
const STACK_BYTES: usize = 8 << 20;

/// A caller's work, borrowed: the caller waits until it has run, so what it
/// points at outlives the run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the kernel's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

struct Handoff {
    jobs: SyncSender<Job>,
    done: Receiver<()>,
}

static KERNEL: OnceLock<Mutex<Handoff>> = OnceLock::new();

thread_local! {
    // Initialized in a const block already; Android's expansion of the macro
    // is what the lint misreads as one that is not.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static ON_KERNEL: Cell<bool> = const { Cell::new(false) };
}

/// Whether this is the kernel's thread: what the allocator asks to place a
/// block in the kernel's region or outside it (memory.rs).
pub fn on_kernel_thread() -> bool {
    ON_KERNEL.with(Cell::get)
}

fn handoff() -> &'static Mutex<Handoff> {
    KERNEL.get_or_init(|| {
        let (jobs, next) = sync_channel::<Job>(1);
        let (finished, done) = sync_channel::<()>(1);
        std::thread::Builder::new()
            .name("pxl-kernel".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                ON_KERNEL.with(|k| k.set(true));
                for Job(job) in next {
                    // SAFETY: the caller's closure, which it keeps until the
                    // answer below has reached it.
                    unsafe { (*job)() };
                    let _ = finished.send(());
                }
            })
            .or_bug(Errno::EAGAIN);
        Mutex::new(Handoff { jobs, done })
    })
}

/// Runs `f` on the kernel's thread, started on the first call, and returns
/// what it returns once it has.
pub fn on_kernel<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    let h = handoff().lock().or_bug(Errno::ENOTRECOVERABLE);
    let mut f = Some(f);
    let mut out = None;
    let mut run = || out = f.take().map(|f| f());
    let run: *mut (dyn FnMut() + Send + '_) = &mut run;
    // SAFETY: the borrow is made `'static` only to cross to the kernel's
    // thread; this call waits for the run to finish before `run` goes.
    let run: *mut (dyn FnMut() + Send + 'static) = unsafe { core::mem::transmute(run) };
    h.jobs.send(Job(run)).or_bug(Errno::EPIPE);
    h.done.recv().or_bug(Errno::EPIPE);
    drop(h);
    out.or_bug(Errno::ENOTRECOVERABLE)
}
