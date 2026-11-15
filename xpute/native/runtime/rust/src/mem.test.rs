// xpute-native-runtime/mem.test.rs

//! The guest's memory of a test that is a program of its own: the runtime's,
//! as a program's `native_abi!` exports it, at the most a u32 offset reaches,
//! on a page, with no share kept from the heap.

/// # Safety
/// `len` and `share` are each null or a writable word.
#[unsafe(no_mangle)]
unsafe extern "C" fn xpute_mem(len: *mut u32, share: *mut u32) -> *mut u8 {
    // SAFETY: the caller's words.
    unsafe { xpute_native_runtime::native::mem(!4095, 0, len, share) }
}
