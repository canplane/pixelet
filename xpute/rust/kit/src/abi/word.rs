// xpute-kit/abi/word.rs

//! Bit fields in 32- and 64-bit words. The caller keeps `nbit` and `shamt`
//! inside the word.

pub const fn bit64(shamt: u32) -> u64 {
    1u64 << shamt
}
pub const fn bit32(shamt: u32) -> u32 {
    1u32 << shamt
}

pub const fn field_mask64(nbit: u32, shamt: u32) -> u64 {
    let field = if nbit >= 64 { u64::MAX } else { (1u64 << nbit) - 1 };
    if shamt >= 64 {
        0
    } else {
        field << shamt
    }
}
pub const fn field_mask32(nbit: u32, shamt: u32) -> u32 {
    let field = if nbit >= 32 { u32::MAX } else { (1u32 << nbit) - 1 };
    if shamt >= 32 {
        0
    } else {
        field << shamt
    }
}

pub const fn field_get64(x: u64, shamt: u32, mask: u64) -> u64 {
    (x >> shamt) & mask
}
pub const fn field_get32(x: u32, shamt: u32, mask: u32) -> u32 {
    (x >> shamt) & mask
}

pub const fn field_set64(x: u64, shamt: u32, mask: u64, v: u64) -> u64 {
    (x & !(mask << shamt)) | ((v & mask) << shamt)
}
pub const fn field_set32(x: u32, shamt: u32, mask: u32, v: u32) -> u32 {
    (x & !(mask << shamt)) | ((v & mask) << shamt)
}

/// The low `nbit` bits of `x`, sign-extended.
pub const fn as_int_n64(nbit: u32, x: u64) -> i64 {
    ((x << (64 - nbit)) as i64) >> (64 - nbit)
}
pub const fn as_int_n32(nbit: u32, x: u32) -> i32 {
    ((x << (32 - nbit)) as i32) >> (32 - nbit)
}
