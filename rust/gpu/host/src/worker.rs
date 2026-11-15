// pxl-gpu-host/worker.rs

use std::sync::mpsc::{Receiver, Sender, channel};

/// A `Worker` is a thread of the host's own for what a browser does apart
/// from the page. It answers its jobs in the order it was sent them, and a
/// later poll lands the answers.
pub(super) struct Worker<J, A> {
    pub(super) ask: Sender<J>,
    pub(super) answers: Receiver<A>,
}

impl<J: Send + 'static, A: Send + 'static> Worker<J, A> {
    /// The thread calls `wake` after each answer, since nothing else asks for
    /// the poll that lands it.
    pub(super) fn new(wake: fn(), work: impl Fn(J) -> A + Send + 'static) -> Worker<J, A> {
        let (ask, jobs) = channel::<J>();
        let (answer, answers) = channel();
        std::thread::spawn(move || {
            for job in jobs {
                if answer.send(work(job)).is_err() {
                    return;
                }
                wake();
            }
        });
        Worker { ask, answers }
    }
}
