// pxl-core/link/life.rs

//! What the app's lifecycle (xpute wit/life/life.wit) does in the core: being
//! hidden, the last moment a save is sure to happen, and being shown again.
//! link/export.rs implements the interface's methods, a UI's attach among them.

use core::future::poll_fn;
use core::task::{Poll, Waker};

use pxl_global::Global;

use crate::domain::layer::object::repository;

/// The object layer is written.
pub(super) fn on_hidden() {
    repository::save();
}

/// How many times the app has been shown, and what awaits the next.
#[derive(Default)]
struct Shown {
    cnt: u32,
    waker: Option<Waker>,
}

static SHOWN: Global<Shown> = Global::new();

fn shown() -> &'static mut Shown {
    SHOWN.get(Shown::default)
}

pub(super) fn on_shown() {
    let s = shown();
    s.cnt = s.cnt.wrapping_add(1);
    if let Some(w) = s.waker.take() {
        w.wake();
    }
}

/// How many times the app has been shown so far, for `shown_after`.
pub(super) fn show_cnt() -> u32 {
    shown().cnt
}

/// Waits until the app has been shown since `show_cnt` answered `since`.
pub(super) async fn shown_after(since: u32) {
    poll_fn(|cx| {
        let s = shown();
        if s.cnt != since {
            return Poll::Ready(());
        }
        s.waker = Some(cx.waker().clone());
        Poll::Pending
    })
    .await;
}
