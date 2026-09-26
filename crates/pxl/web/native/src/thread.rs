// pxl-native/thread.rs

//! The kernel's own thread. The app calls in from whichever thread its shell
//! runs on — React Native's JavaScript thread, whose stack is 1 MiB — and a
//! turn that draws through wgpu can take more than that
//! (issue/20260926-a-device-s-kernel-on-a-thread-of-its-own.md). So every
//! call that reaches the kernel is handed to this thread, with a stack the
//! kernel sizes, and the caller waits for it: one of the two runs at a time,
//! and the kernel is single-threaded on this thread rather than the
//! caller's.

use std::sync::mpsc::{channel, sync_channel, Sender};
use std::sync::OnceLock;

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The kernel's stack: the main thread's default on Linux and macOS. Not
/// derived — how deep wgpu's internals go has no bound this crate can state.
const STACK_BYTES: usize = 8 << 20;

type Job = Box<dyn FnOnce() + Send>;

static KERNEL: OnceLock<Sender<Job>> = OnceLock::new();

/// Runs `f` on the kernel's thread, started on the first call, and returns
/// what it returns once it has.
pub fn on_kernel<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    let jobs = KERNEL.get_or_init(|| {
        let (jobs, next) = channel::<Job>();
        std::thread::Builder::new()
            .name("pxl-kernel".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                for job in next {
                    job();
                }
            })
            .or_bug(Errno::EAGAIN);
        jobs
    });
    let (done, answer) = sync_channel(1);
    jobs.send(Box::new(move || {
        // The caller is waiting on the answer, so it is there to take it.
        let _ = done.send(f());
    }))
    .or_bug(Errno::EPIPE);
    answer.recv().or_bug(Errno::EPIPE)
}
