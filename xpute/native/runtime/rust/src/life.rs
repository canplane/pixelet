// xpute-native-runtime/life.rs

//! The app's lifecycle, told to a guest that serves it (wit/life/life.wit), as
//! life.ts tells a page's: the platform's hidden and shown come through the
//! queue. A guest that serves none answers ENOENT, and is told nothing more.

use crate::abi;

/// Tells the guest `hidden` or `shown`.
pub(crate) fn tell(shown: bool) {
    // The guest's answer is its own, as a page's is (life.ts).
    let _ = if shown {
        abi::life::life::shown()
    } else {
        abi::life::life::hidden()
    };
}
