// pxl-core/link/life.rs

//! The app's lifecycle, which the core serves (xpute wit/life.wit): hidden,
//! the last moment a save is sure to happen, and shown again.

use core::future::poll_fn;
use core::task::{Poll, Waker};
use std::cell::Cell;

use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::domain::layer::object::repository;

/// The objects are written, and the position is no longer watched while
/// nothing shows it.
pub(super) fn on_hidden() {
    geolocation::geolocation_visibility(false);
    repository::save();
}

thread_local! {
    /// How many times the app has been shown, and what awaits the next.
    static SHOWN: Cell<u32> = const { Cell::new(0) };
    static WAKER: Cell<Option<Waker>> = const { Cell::new(None) };
}

pub(super) fn on_shown() {
    geolocation::geolocation_visibility(true);
    device_orientation::orient_visible();
    SHOWN.set(SHOWN.get().wrapping_add(1));
    if let Some(w) = WAKER.take() {
        w.wake();
    }
}

/// How many times the app has been shown so far, for `shown_after`.
pub(super) fn shows() -> u32 {
    SHOWN.get()
}

/// Waits until the app has been shown since `shows` answered `since`.
pub(super) async fn shown_after(since: u32) {
    poll_fn(|cx| {
        if SHOWN.get() != since {
            return Poll::Ready(());
        }
        WAKER.set(Some(cx.waker().clone()));
        Poll::Pending
    })
    .await;
}
