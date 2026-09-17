// xpute_core/io/scalar.test.rs
// (no pair: scalar.ts has no test file)

use super::*;
use crate::kit::encoding::bytes_to_hex;
use crate::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/io/scalar.tsv");

#[test]
fn scalar_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    let buf: Bytes = TypedArray::new(64);
    let mut st = stream(buf.clone(), 0);
    putu8(&mut st, 0xab);
    puti8(&mut st, -2);
    putu16(&mut st, 0xbeef);
    puti16(&mut st, -12345);
    putu32(&mut st, 0xdeadbeef);
    puti32(&mut st, -123456789);
    putu64(&mut st, 0x0123456789abcdef);
    puti64(&mut st, -9876543210);
    putf32(&mut st, 1.1);
    putf64(&mut st, -0.1);
    putu96(&mut st, 1, 2, 0xffffffff);
    assert_eq!(bytes_to_hex(&buf), v.s("written"));
    assert_eq!(tell(&st), v.i32("tell"));
    assert_eq!(pack96(1, 2, 3).to_string(), v.s("pack96"));
    assert_eq!(unpack96((1u128 << 90) + 12345), [v.u32("unpack96.0"), v.u32("unpack96.1"), v.u32("unpack96.2")]);
    assert_eq!(pack128(0xffffffffffffffff, 7).to_string(), v.s("pack128"));
    assert_eq!(unpack128((5u128 << 64) + 9), [v.u64("unpack128.0"), v.u64("unpack128.1")]);
    // Reading back what was written.
    let mut r = stream(buf, 0);
    assert_eq!((getu8(&mut r), geti8(&mut r), getu16(&mut r), geti16(&mut r)), (0xab, -2, 0xbeef, -12345));
    assert_eq!(
        (getu32(&mut r), geti32(&mut r), getu64(&mut r), geti64(&mut r)),
        (0xdeadbeef, -123456789, 0x0123456789abcdef, -9876543210)
    );
    assert_eq!((getf32(&mut r), getf64(&mut r), getu96(&mut r)), (1.1, -0.1, [1, 2, 0xffffffff]));
}
