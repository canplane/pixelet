// xpute-native-runtime/life.rs

//! The app's lifecycle, told to a guest that serves it (wit/life/life.wit), as
//! life.ts tells a page's: the platform's hidden and shown come through the
//! queue, and a UI attached at each start (remote.rs `accept`). The guest's
//! answer is its own: no error it answers raises anything, ENOENT where it
//! serves none and EPIPE where it trapped among them, as on a page (life.ts
//! `take_answer`).

use crate::abi;

/// Tells the guest `hidden` or `shown`.
pub(crate) fn tell(shown: bool) {
    // The guest's answer is its own, as a page's is (life.ts).
    let _ = if shown {
        abi::life::shown()
    } else {
        abi::life::hidden()
    };
}

/// Tells the guest a UI attached.
pub(crate) fn attached() {
    let _ = abi::life::attached();
}
