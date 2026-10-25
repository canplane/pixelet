// pxl-kernel/profiler/word.rs

//! A record as profiler.json lays it out, word for word, checked at compile time.

use core::mem::size_of;

/// A record laid out as a profiler.json section. Implemented only through
/// `words!`, which checks every field's offset at compile time.
///
/// # Safety
/// The implementor is `#[repr(C)]` and all f64 (or such records), no padding.
pub unsafe trait Words: Sized {}

pub fn put<T: Words>(out: &mut [f64], at: u32, value: &T) {
    // SAFETY: `Words` guarantees plain f64s laid end to end.
    let words = unsafe {
        core::slice::from_raw_parts(
            std::ptr::from_ref::<T>(value).cast::<f64>(),
            size_of::<T>() / 8,
        )
    };
    let at = at as usize;
    out[at..at + words.len()].copy_from_slice(words);
}

macro_rules! words {
    ($t:ty, $words:expr, $($($field:ident).+ => $at:expr),+ $(,)?) => {
        const _: () = assert!(size_of::<$t>() == $words as usize * 8, concat!(stringify!($t), " is not the size profiler.json lays out"));
        $(const _: () = assert!(offset_of!($t, $($field).+) == $at as usize * 8, concat!(stringify!($t), " puts ", stringify!($($field).+), " off the word profiler.json gives it"));)+
        // SAFETY: the two assertions above are the claim.
        unsafe impl Words for $t {}
    };
}
pub(crate) use words;
