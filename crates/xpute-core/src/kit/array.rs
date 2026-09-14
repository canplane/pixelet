// @xpute/core/kit/array.ts

//! AB-backed typed array helpers: the pair of kit/array.ts.
//!
//! JavaScript's typed arrays are the language's; this is what they read as
//! here, once, so every port over them is line for line:
//!
//! - `ArrayBuffer` is a handle to bytes — its own (`new`), or a range of a
//!   memory it does not own (`from_raw`, the module's linear memory). Two
//!   handles to one buffer compare equal by identity (`is`), as `===` does.
//!   A `SharedArrayBuffer` is an `ArrayBuffer` made `shared`.
//! - `TypedArray<T>` is a view: a buffer, a byte offset and a length, over
//!   one element type. Views alias, as they do there; an element is read
//!   and written through the buffer, and the element type must be aligned
//!   in the buffer as a typed array requires.
//! - A constructor is a type: `new Uint8Array(n)` is `TypedArray::<u8>::new(n)`,
//!   and a `TypedArrayCtor<T>` parameter is the type parameter `T`.
//!   `BYTES_PER_ELEMENT` is the element's.
//! - The type unions (`TypedArray`, `BigTypedArray`, `AnyTypedArray`) are
//!   the element traits: `NumberElement` for the Number family,
//!   `BigElement` for the BigInt family, `AnyElement` for both. A runtime
//!   guard over `unknown` (`is_ta`, `is_sab`) takes a `Vals`, the sum of
//!   what the boundary functions accept.
//! - `ArrayBufferView` is any view with its type erased: what `give` and
//!   `to_bytes` take.
//!
//! Number and BigInt typed arrays are supported in separate sections, as
//! there. Inputs must satisfy the function contracts; contract violations
//! panic where the TypeScript throws a RangeError.

#![allow(non_camel_case_types)]

use core::cell::UnsafeCell;
use std::rc::Rc;

// ============ The buffer ============

struct Storage {
    ptr: *mut u8,
    byte_length: usize,
    /// The bytes when the buffer owns them; none for a range of a memory.
    _owned: Option<Box<[UnsafeCell<u8>]>>,
    shared: bool,
}

/// A handle to bytes; clones are the same buffer.
#[derive(Clone)]
pub struct ArrayBuffer(Rc<Storage>);

impl ArrayBuffer {
    /// `new ArrayBuffer(byte_length)`: zeroed bytes of its own.
    pub fn new(byte_length: usize) -> ArrayBuffer {
        Self::with(byte_length, false)
    }

    /// `new SharedArrayBuffer(byte_length)`.
    pub fn new_shared(byte_length: usize) -> ArrayBuffer {
        Self::with(byte_length, true)
    }

    fn with(byte_length: usize, shared: bool) -> ArrayBuffer {
        let owned: Box<[UnsafeCell<u8>]> = (0..byte_length).map(|_| UnsafeCell::new(0)).collect();
        let ptr = owned.as_ptr() as *mut u8;
        ArrayBuffer(Rc::new(Storage {
            ptr,
            byte_length,
            _owned: Some(owned),
            shared,
        }))
    }

    /// A buffer over `byte_length` bytes at `ptr` that it does not own:
    /// `WebAssembly.Memory.buffer`, the memory the module lives in.
    ///
    /// # Safety
    /// The bytes must stay valid, and never move, for as long as any handle
    /// or view of the buffer is used.
    pub unsafe fn from_raw(ptr: *mut u8, byte_length: usize) -> ArrayBuffer {
        ArrayBuffer(Rc::new(Storage {
            ptr,
            byte_length,
            _owned: None,
            shared: false,
        }))
    }

    pub fn byte_length(&self) -> usize {
        self.0.byte_length
    }

    pub fn is_shared(&self) -> bool {
        self.0.shared
    }

    /// `a === b`: the same buffer, not equal bytes.
    pub fn is(&self, other: &ArrayBuffer) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// The buffer's identity as a number, for a map keyed by buffer.
    pub fn id(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.0.ptr
    }
}

// ============ The elements ============

/// An element type of any typed array: the pair of `AnyTypedArrayCtor`'s
/// `BYTES_PER_ELEMENT`, on the type a constructor names.
pub trait AnyElement: Copy + Default + PartialEq + core::fmt::Debug + 'static {
    const BYTES_PER_ELEMENT: usize;
    const KIND: Kind;
}

/// The Number family (`TypedArray`, `TypedArrayCtor`).
pub trait NumberElement: AnyElement {
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
}

/// The BigInt family (`BigTypedArray`, `BigTypedArrayCtor`).
pub trait BigElement: AnyElement {
    fn to_i128(self) -> i128;
    fn from_i128(v: i128) -> Self;
}

/// Which typed array a view is, once its element type is erased.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
    U64,
    I64,
}

impl Kind {
    pub fn bytes_per_element(self) -> usize {
        match self {
            Kind::U8 | Kind::I8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 | Kind::I32 | Kind::F32 => 4,
            Kind::F64 | Kind::U64 | Kind::I64 => 8,
        }
    }

    pub fn is_big(self) -> bool {
        matches!(self, Kind::U64 | Kind::I64)
    }
}

macro_rules! number_element {
    ($t:ty, $kind:ident, $cast:ident) => {
        impl AnyElement for $t {
            const BYTES_PER_ELEMENT: usize = core::mem::size_of::<$t>();
            const KIND: Kind = Kind::$kind;
        }
        impl NumberElement for $t {
            fn to_f64(self) -> f64 {
                self as f64
            }
            fn from_f64(v: f64) -> $t {
                $cast(v)
            }
        }
    };
}

// A typed array store converts as JS does: ToInt32/ToUint32 modulo 2^32
// (NaN to 0, the fraction dropped), then the low bits; a float store rounds.
fn to_int_bits(v: f64) -> i64 {
    if !v.is_finite() {
        0
    } else {
        (v.trunc() % 4_294_967_296.0) as i64
    }
}
fn store_u8(v: f64) -> u8 {
    to_int_bits(v) as u8
}
fn store_i8(v: f64) -> i8 {
    to_int_bits(v) as i8
}
fn store_u16(v: f64) -> u16 {
    to_int_bits(v) as u16
}
fn store_i16(v: f64) -> i16 {
    to_int_bits(v) as i16
}
fn store_u32(v: f64) -> u32 {
    to_int_bits(v) as u32
}
fn store_i32(v: f64) -> i32 {
    to_int_bits(v) as i32
}
fn store_f32(v: f64) -> f32 {
    v as f32
}
fn store_f64(v: f64) -> f64 {
    v
}

number_element!(u8, U8, store_u8);
number_element!(i8, I8, store_i8);
number_element!(u16, U16, store_u16);
number_element!(i16, I16, store_i16);
number_element!(u32, U32, store_u32);
number_element!(i32, I32, store_i32);
number_element!(f32, F32, store_f32);
number_element!(f64, F64, store_f64);

macro_rules! big_element {
    ($t:ty, $kind:ident) => {
        impl AnyElement for $t {
            const BYTES_PER_ELEMENT: usize = 8;
            const KIND: Kind = Kind::$kind;
        }
        impl BigElement for $t {
            fn to_i128(self) -> i128 {
                self as i128
            }
            // BigInt.asUintN(64, v) / asIntN(64, v): the low 64 bits.
            fn from_i128(v: i128) -> $t {
                v as $t
            }
        }
    };
}

big_element!(u64, U64);
big_element!(i64, I64);

// ============ The view ============

/// A typed array: a view of `length` elements of `T` at `byte_offset` of
/// `buffer`.
#[derive(Clone)]
pub struct TypedArray<T: AnyElement> {
    buffer: ArrayBuffer,
    byte_offset: usize,
    length: usize,
    _t: core::marker::PhantomData<T>,
}

impl<T: AnyElement> TypedArray<T> {
    /// `new Ctor(length)`: a zeroed array over a buffer of its own.
    pub fn new(length: usize) -> TypedArray<T> {
        Self::over(ArrayBuffer::new(length * T::BYTES_PER_ELEMENT), 0, length)
    }

    /// `new Ctor(buffer, byte_offset, length)`: a view over `buffer`.
    /// Panics where the constructor throws: an offset not a multiple of the
    /// element size, or a window past the buffer's end.
    pub fn over(buffer: ArrayBuffer, byte_offset: usize, length: usize) -> TypedArray<T> {
        assert!(
            byte_offset.is_multiple_of(T::BYTES_PER_ELEMENT),
            "start offset of {:?} should be a multiple of {}",
            T::KIND,
            T::BYTES_PER_ELEMENT
        );
        assert!(byte_offset + length * T::BYTES_PER_ELEMENT <= buffer.byte_length(), "invalid typed array length: {}", length);
        TypedArray {
            buffer,
            byte_offset,
            length,
            _t: core::marker::PhantomData,
        }
    }

    /// `new Ctor(buffer)`: the whole buffer.
    pub fn whole(buffer: ArrayBuffer) -> TypedArray<T> {
        let length = buffer.byte_length() / T::BYTES_PER_ELEMENT;
        assert!(
            buffer.byte_length().is_multiple_of(T::BYTES_PER_ELEMENT),
            "byte length of {:?} should be a multiple of {}",
            T::KIND,
            T::BYTES_PER_ELEMENT
        );
        Self::over(buffer, 0, length)
    }

    /// `new Ctor(elements)`: a copy of `elements` in a buffer of its own.
    pub fn from(elements: &[T]) -> TypedArray<T> {
        let out = Self::new(elements.len());
        out.as_mut_slice().copy_from_slice(elements);
        out
    }

    pub fn buffer(&self) -> &ArrayBuffer {
        &self.buffer
    }

    pub fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub fn byte_length(&self) -> usize {
        self.length * T::BYTES_PER_ELEMENT
    }

    pub fn length(&self) -> usize {
        self.length
    }

    fn ptr(&self) -> *mut T {
        // SAFETY: `over` checked the window against the buffer, whose bytes
        // outlive every handle to it.
        unsafe { self.buffer.as_ptr().add(self.byte_offset) as *mut T }
    }

    /// The elements as a slice. Views alias, as JavaScript's do: two views
    /// of one buffer see each other's writes, and a slice of one must not
    /// be held across a write through another.
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the window is inside the buffer and the offset is aligned
        // (`over`); aliasing between views is the caller's, as in JS.
        unsafe { core::slice::from_raw_parts(self.ptr(), self.length) }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn as_mut_slice(&self) -> &mut [T] {
        // SAFETY: as `as_slice`.
        unsafe { core::slice::from_raw_parts_mut(self.ptr(), self.length) }
    }

    pub fn get(&self, index: usize) -> T {
        self.as_slice()[index]
    }

    pub fn put(&self, index: usize, value: T) {
        self.as_mut_slice()[index] = value;
    }

    /// `view.set(other)`: copies `other` in from the start.
    pub fn set(&self, other: &TypedArray<T>) {
        self.set_at(other, 0);
    }

    /// `view.set(other, offset)`.
    pub fn set_at(&self, other: &TypedArray<T>, offset: usize) {
        assert!(offset + other.length <= self.length, "offset is out of bounds");
        // The windows may overlap in one buffer; a typed array's `set` copies
        // as if through a temporary.
        let src = other.as_slice().to_vec();
        self.as_mut_slice()[offset..offset + src.len()].copy_from_slice(&src);
    }

    pub fn fill(&self, value: T) {
        self.as_mut_slice().fill(value);
    }

    /// `view.subarray(begin, end)`: a view of the same buffer.
    pub fn subarray(&self, begin: usize, end: usize) -> TypedArray<T> {
        assert!(begin <= end && end <= self.length, "subarray out of bounds");
        Self::over(self.buffer.clone(), self.byte_offset + begin * T::BYTES_PER_ELEMENT, end - begin)
    }

    /// The view with its element type erased.
    pub fn view(&self) -> ArrayBufferView {
        ArrayBufferView {
            buffer: self.buffer.clone(),
            byte_offset: self.byte_offset,
            byte_length: self.byte_length(),
            kind: T::KIND,
        }
    }

    /// A typed view back from an erased one of this element type.
    pub fn of(view: &ArrayBufferView) -> Option<TypedArray<T>> {
        (view.kind == T::KIND).then(|| Self::over(view.buffer.clone(), view.byte_offset, view.byte_length / T::BYTES_PER_ELEMENT))
    }

    /// Whether `other` is the same view: the same buffer, offset and length.
    pub fn is(&self, other: &TypedArray<T>) -> bool {
        self.buffer.is(&other.buffer) && self.byte_offset == other.byte_offset && self.length == other.length
    }
}

impl<T: AnyElement> core::ops::Index<usize> for TypedArray<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.as_slice()[i]
    }
}

impl<T: AnyElement> core::ops::IndexMut<usize> for TypedArray<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.as_mut_slice()[i]
    }
}

/// Two views are equal when their elements are, as `assertEquals` reads
/// typed arrays.
impl<T: AnyElement> PartialEq for TypedArray<T> {
    fn eq(&self, other: &TypedArray<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: AnyElement> core::fmt::Debug for TypedArray<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

/// Any typed array with its element type erased: the DOM's `ArrayBufferView`.
#[derive(Clone)]
pub struct ArrayBufferView {
    pub buffer: ArrayBuffer,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub kind: Kind,
}

impl ArrayBufferView {
    pub fn length(&self) -> usize {
        self.byte_length / self.kind.bytes_per_element()
    }
}

/// Equal when of one kind with equal bytes.
impl PartialEq for ArrayBufferView {
    fn eq(&self, other: &ArrayBufferView) -> bool {
        self.kind == other.kind && to_bytes(self) == to_bytes(other)
    }
}

impl core::fmt::Debug for ArrayBufferView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}[{}]", self.kind, self.length())
    }
}

/// What a boundary function accepts in place of `unknown`: a buffer, a
/// view, or a plain array of numbers or of BigInts.
pub enum Vals<'a> {
    ArrayBuffer(ArrayBuffer),
    View(ArrayBufferView),
    ArrayLike(&'a [f64]),
    BigArrayLike(&'a [i128]),
}

// ============ Number ============

// ---- Canonical aliases ----

pub type U8Array = TypedArray<u8>;
pub type I8Array = TypedArray<i8>;
pub type U16Array = TypedArray<u16>;
pub type I16Array = TypedArray<i16>;
pub type U32Array = TypedArray<u32>;
pub type I32Array = TypedArray<i32>;
pub type F32Array = TypedArray<f32>;
pub type F64Array = TypedArray<f64>;

// ---- Typed array meta ----

/// The Number family, as a bound: `T: TypedArrayCtor` is the constructor
/// of a `TypedArray<T>`.
pub trait TypedArrayCtor: NumberElement {}
impl<T: NumberElement> TypedArrayCtor for T {}
/// The element of a Number typed array.
pub type ElementOf<T> = T;

// ---- View normalization ----

/// True if vals is an ArrayBuffer.
pub fn is_ab(vals: &Vals) -> bool {
    matches!(vals, Vals::ArrayBuffer(b) if !b.is_shared())
}

/// True if vals is a SharedArrayBuffer.
pub fn is_sab(vals: &Vals) -> bool {
    matches!(vals, Vals::ArrayBuffer(b) if b.is_shared())
}

/// True if vals is one of the supported Number typed arrays.
pub fn is_ta(vals: &Vals) -> bool {
    matches!(vals, Vals::View(v) if !v.kind.is_big())
}

/// True if vals is a Number typed array backed by ArrayBuffer.
pub fn is_ta_ab(vals: Option<&Vals>) -> bool {
    matches!(vals, Some(Vals::View(v)) if !v.kind.is_big() && !v.buffer.is_shared())
}

/// True if vals is a Number typed array backed by SharedArrayBuffer.
pub fn is_ta_sab(vals: Option<&Vals>) -> bool {
    matches!(vals, Some(Vals::View(v)) if !v.kind.is_big() && v.buffer.is_shared())
}

/// Normalize common boundary inputs into an AB-backed T.
/// Accepts ArrayBuffer, Number typed array views, and array-like values.
/// Invalid inputs are treated as contract violations and may panic.
/// - exact T over AB: return as-is
/// - ArrayBuffer: zero-copy wrap
/// - other Number typed array view over AB: zero-copy rewrap
/// - Number typed array view over SAB: copy
/// - plain ArrayLike: copy
pub fn as_ta_ab<T: TypedArrayCtor>(vals: &Vals) -> TypedArray<T> {
    match vals {
        // exact type over AB — the rewrap below is the same view.
        Vals::ArrayBuffer(buf) if !buf.is_shared() => TypedArray::whole(buf.clone()),
        Vals::View(v) if !v.kind.is_big() => {
            let len = v.length();
            // AB-backed: preserve current window zero-copy
            if !v.buffer.is_shared() {
                return TypedArray::over(v.buffer.clone(), v.byte_offset, len);
            }
            // SAB-backed: copy into fresh AB-backed storage
            let out = TypedArray::<T>::new(len);
            out.set(&TypedArray::<T>::over(v.buffer.clone(), v.byte_offset, len));
            out
        }
        Vals::ArrayLike(a) => {
            let out = TypedArray::<T>::new(a.len());
            for (i, &x) in a.iter().enumerate() {
                out.put(i, T::from_f64(x));
            }
            out
        }
        _ => panic!("as_ta_ab: not an ArrayBuffer, a Number typed array or an array of numbers"),
    }
}

/// Ensure vals is an instance of T.
/// - exact T: return as-is (zero-copy)
/// - plain ArrayLike: copy into fresh AB-backed T
pub fn to_ta<T: TypedArrayCtor>(arr: &Vals) -> TypedArray<T> {
    match arr {
        Vals::View(v) if v.kind == T::KIND => TypedArray::over(v.buffer.clone(), v.byte_offset, v.length()),
        _ => as_ta_ab::<T>(arr),
    }
}

/// Copy a typed array window into fresh AB-backed T.
/// This is the forced-copy counterpart to as_ta_ab().
pub fn copy_ab_view<T: TypedArrayCtor>(buf: &ArrayBuffer, off: u32, len: u32) -> TypedArray<T> {
    let out = TypedArray::<T>::new(len as usize);
    out.set(&TypedArray::<T>::over(buf.clone(), off as usize, len as usize));
    out
}

// ---- Bytes (U8Array) ----

/// Canonical byte carrier view used across the codebase.
pub type Bytes = U8Array;

/// True if vals is canonical AB-backed bytes.
pub fn is_bytes(vals: &Vals) -> bool {
    matches!(vals, Vals::View(v) if v.kind == Kind::U8 && !v.buffer.is_shared())
}

/// Thin byte-specialized wrapper over as_ta_ab(Uint8Array).
pub fn as_bytes(vals: &Vals) -> Bytes {
    as_ta_ab::<u8>(vals)
}

/// Zero-copy cast of any typed array view into a byte view (U8Array).
/// Preserves the source view window (byteOffset/byteLength).
pub fn to_bytes(arr: &ArrayBufferView) -> Bytes {
    TypedArray::over(arr.buffer.clone(), arr.byte_offset, arr.byte_length)
}

/// Copy a byte window into fresh AB-backed bytes.
pub fn copy_bytes(buf: &ArrayBuffer, off: u32, nbyte: u32) -> Bytes {
    copy_ab_view::<u8>(buf, off, nbyte)
}

// ============ BigInt ============

// ---- Canonical aliases ----

pub type U64Array = TypedArray<u64>;
pub type I64Array = TypedArray<i64>;

// ---- Typed array meta ----

/// The BigInt family's view.
pub type BigTypedArray<T> = TypedArray<T>;
/// The BigInt family, as a bound.
pub trait BigTypedArrayCtor: BigElement {}
impl<T: BigElement> BigTypedArrayCtor for T {}
/// The element of a BigInt typed array.
pub type BigElementOf<T> = T;

// ---- View normalization ----

/// True if vals is one of the supported BigInt typed arrays.
pub fn is_bta(vals: &Vals) -> bool {
    matches!(vals, Vals::View(v) if v.kind.is_big())
}

/// True if vals is a BigInt typed array backed by ArrayBuffer.
pub fn is_bta_ab(vals: Option<&Vals>) -> bool {
    matches!(vals, Some(Vals::View(v)) if v.kind.is_big() && !v.buffer.is_shared())
}

/// True if vals is a BigInt typed array backed by SharedArrayBuffer.
pub fn is_bta_sab(vals: Option<&Vals>) -> bool {
    matches!(vals, Some(Vals::View(v)) if v.kind.is_big() && v.buffer.is_shared())
}

/// Normalize common boundary inputs into an AB-backed BigInt T.
/// Accepts ArrayBuffer, BigInt typed array views, and array-like values.
/// Invalid inputs are treated as contract violations and may panic.
/// - exact T over AB: return as-is
/// - ArrayBuffer: zero-copy wrap
/// - other BigInt typed array view over AB: zero-copy rewrap
/// - BigInt typed array view over SAB: copy
/// - plain ArrayLike: copy
pub fn as_bta_ab<T: BigTypedArrayCtor>(vals: &Vals) -> TypedArray<T> {
    match vals {
        Vals::ArrayBuffer(buf) if !buf.is_shared() => TypedArray::whole(buf.clone()),
        Vals::View(v) if v.kind.is_big() => {
            let len = v.length();
            // AB-backed: preserve current window zero-copy
            if !v.buffer.is_shared() {
                return TypedArray::over(v.buffer.clone(), v.byte_offset, len);
            }
            // SAB-backed: copy into fresh AB-backed storage
            let out = TypedArray::<T>::new(len);
            out.set(&TypedArray::<T>::over(v.buffer.clone(), v.byte_offset, len));
            out
        }
        Vals::BigArrayLike(a) => {
            let out = TypedArray::<T>::new(a.len());
            for (i, &x) in a.iter().enumerate() {
                out.put(i, T::from_i128(x));
            }
            out
        }
        _ => panic!("as_bta_ab: not an ArrayBuffer, a BigInt typed array or an array of BigInts"),
    }
}

/// Ensure vals is an instance of BigInt T.
/// - exact T: return as-is (zero-copy)
/// - plain ArrayLike: copy into fresh AB-backed T
pub fn to_bta<T: BigTypedArrayCtor>(arr: &Vals) -> TypedArray<T> {
    match arr {
        Vals::View(v) if v.kind == T::KIND => TypedArray::over(v.buffer.clone(), v.byte_offset, v.length()),
        _ => as_bta_ab::<T>(arr),
    }
}

/// Copy a BigInt typed array window into fresh AB-backed T.
/// This is the forced-copy counterpart to as_bta_ab().
pub fn copy_bta_view<T: BigTypedArrayCtor>(buf: &ArrayBuffer, off: u32, len: u32) -> TypedArray<T> {
    let out = TypedArray::<T>::new(len as usize);
    out.set(&TypedArray::<T>::over(buf.clone(), off as usize, len as usize));
    out
}

// ============ Any ============

/// Minimal typed-array-like view shape used by generic packet readers.
pub type AnyTypedArray<T> = TypedArray<T>;

/// The element of any typed array.
pub type AnyElementOf<T> = T;

/// Minimal ctor shape for zero-copy typed-array view rewraps.
pub trait AnyTypedArrayCtor: AnyElement {}
impl<T: AnyElement> AnyTypedArrayCtor for T {}
