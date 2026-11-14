// pxl-core/link/life.rs

//! The app's lifecycle, which the core serves (xpute wit/life/life.wit):
//! hidden, the last moment a save is sure to happen, and shown again.

use core::future::poll_fn;
use core::task::{Poll, Waker};

use pxl_global::Global;

use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::domain::layer::object::repository;

/// The objects are written, and the position is no longer watched while
/// nothing shows it.
pub(super) fn on_hidden() {
    geolocation::geolocation_visibility(false);
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
    geolocation::geolocation_visibility(true);
    device_orientation::orient_visible();
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
