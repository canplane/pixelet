// xpute-link/cabi.rs

//! A packet's values in the Component Model's Canonical ABI layout: each at
//! its type's alignment, little-endian, a record's fields in order, a variant
//! its discriminant and then its payload. The packet is a call's params as one
//! tuple, as the ABI lays a spilled tuple in memory, or an answer's value.
//! Where the ABI puts a string's or a list's bytes in the other side's memory
//! through its `cabi_realloc`, the packet holds them after its fixed part, and
//! their pointer is an offset from the packet's start: the frame's packet is
//! the one copy. A packet that does not hold what its type says is refused
//! (EBADMSG), where the ABI traps.

use core::marker::PhantomData;

use xpute_kit::status::errno::Errno;

/// A number a list holds, whose bytes, every one set, are its value
/// little-endian, as on every target xpute runs on: a list of them crosses as
/// its bytes, in one copy.
pub trait Number: Copy + Default {}

macro_rules! number {
    ($($t:ty)*) => {$(impl Number for $t {})*};
}
number!(u8 i8 u16 i16 u32 i32 u64 i64 f32 f64);

/// Writes a packet into the bytes it is given: its fixed part first
/// (`start`), then the bytes of its strings and lists past it. A packet past
/// the bytes it is given spoils, and `finish` answers EMSGSIZE; a writer
/// nothing was written by answers 0, no packet. It holds its bytes by
/// address, as a reader does (`Lift`), so a side can hand an object the
/// reader of a frame's args and the writer of its answer over the same bytes,
/// the answer written once the args are read.
pub struct Lower<'a> {
    at: *mut u8,
    cap: usize,
    end: usize,
    overflow: bool,
    _buf: PhantomData<&'a mut [u8]>,
}

impl<'a> Lower<'a> {
    /// A packet of a fixed part `fixed_sz` long, zeroed, in `buf`.
    pub fn new(buf: &'a mut [u8], fixed_sz: usize) -> Lower<'a> {
        // SAFETY: `buf`'s bytes, which the writer borrows.
        let mut w = unsafe { Lower::from_raw_parts(buf.as_mut_ptr(), buf.len()) };
        w.start(fixed_sz);
        w
    }

    /// A writer over the `cap` bytes at `at`, nothing written there yet.
    ///
    /// # Safety
    /// The bytes are writable for `'a`, and nothing but this writer and a
    /// reader over them (`Lift::from_raw_parts`) touches them meanwhile.
    pub unsafe fn from_raw_parts(at: *mut u8, cap: usize) -> Lower<'a> {
        Lower {
            at,
            cap,
            end: 0,
            overflow: false,
            _buf: PhantomData,
        }
    }

    /// Starts the packet with a fixed part `fixed_sz` long, zeroed.
    pub fn start(&mut self, fixed_sz: usize) {
        self.overflow = fixed_sz > self.cap;
        if !self.overflow {
            // SAFETY: within the writer's bytes, as just checked.
            unsafe { self.at.write_bytes(0, fixed_sz) };
        }
        self.end = fixed_sz;
    }

    /// Copies `b` to `at`, within the writer's bytes, as the caller checked;
    /// `b` may lie in them.
    fn copy(&mut self, at: usize, b: &[u8]) {
        // SAFETY: the caller's; the copy may overlap.
        unsafe { core::ptr::copy(b.as_ptr(), self.at.add(at), b.len()) };
    }

    fn put<const N: usize>(&mut self, at: usize, b: [u8; N]) {
        if self.overflow || at.checked_add(N).is_none_or(|end| end > self.cap) {
            self.overflow = true;
            return;
        }
        self.copy(at, &b);
    }

    pub fn bool(&mut self, at: usize, v: bool) {
        self.put(at, [u8::from(v)]);
    }

    pub fn u8(&mut self, at: usize, v: u8) {
        self.put(at, v.to_le_bytes());
    }

    pub fn i8(&mut self, at: usize, v: i8) {
        self.put(at, v.to_le_bytes());
    }

    pub fn u16(&mut self, at: usize, v: u16) {
        self.put(at, v.to_le_bytes());
    }

    pub fn i16(&mut self, at: usize, v: i16) {
        self.put(at, v.to_le_bytes());
    }

    pub fn u32(&mut self, at: usize, v: u32) {
        self.put(at, v.to_le_bytes());
    }

    pub fn i32(&mut self, at: usize, v: i32) {
        self.put(at, v.to_le_bytes());
    }

    pub fn u64(&mut self, at: usize, v: u64) {
        self.put(at, v.to_le_bytes());
    }

    pub fn i64(&mut self, at: usize, v: i64) {
        self.put(at, v.to_le_bytes());
    }

    pub fn f32(&mut self, at: usize, v: f32) {
        self.put(at, v.to_le_bytes());
    }

    pub fn f64(&mut self, at: usize, v: f64) {
        self.put(at, v.to_le_bytes());
    }

    /// Takes `len` elements of `sz` bytes at `align` past what is written,
    /// zeroed, and puts their offset and count at `at`: a list's or a
    /// string's pointer and length. It answers where they begin, or none
    /// where they run past the bytes it is given.
    pub fn list(&mut self, at: usize, len: usize, sz: usize, align: usize) -> Option<usize> {
        let base = self.end.next_multiple_of(align);
        let end = base + len * sz;
        if self.overflow || end > self.cap {
            self.overflow = true;
            return None;
        }
        // SAFETY: within the writer's bytes, as just checked.
        unsafe { self.at.add(self.end).write_bytes(0, end - self.end) };
        self.end = end;
        self.u32(at, base as u32);
        self.u32(at + 4, len as u32);
        Some(base)
    }

    /// Puts the numbers `v` past what is written, at their alignment, and
    /// their offset and count at `at`.
    pub fn arr<T: Number>(&mut self, at: usize, v: &[T]) {
        let sz = size_of::<T>();
        // SAFETY: a number's bytes are all set (`Number`).
        let bytes = unsafe { core::slice::from_raw_parts(v.as_ptr().cast::<u8>(), size_of_val(v)) };
        if let Some(base) = self.list(at, v.len(), sz, sz) {
            self.copy(base, bytes);
        }
    }

    pub fn str(&mut self, at: usize, s: &str) {
        self.arr(at, s.as_bytes());
    }

    /// The packet's length, or EMSGSIZE where it is past the bytes it is given.
    pub fn finish(self) -> Result<usize, Errno> {
        if self.overflow {
            Err(Errno::EMSGSIZE)
        } else {
            Ok(self.end)
        }
    }
}

/// Reads a packet's values where its type says they lie, refusing EBADMSG
/// what lies past the packet, a misaligned list, or a string not UTF-8. It
/// holds its packet by address, as a writer does (`Lower`).
#[derive(Clone, Copy)]
pub struct Lift<'a> {
    at: *const u8,
    len: usize,
    _pkt: PhantomData<&'a [u8]>,
}

impl<'a> Lift<'a> {
    pub fn new(pkt: &'a [u8]) -> Lift<'a> {
        // SAFETY: `pkt`'s bytes, which the reader borrows.
        unsafe { Lift::from_raw_parts(pkt.as_ptr(), pkt.len()) }
    }

    /// A reader of the `len` bytes at `at`.
    ///
    /// # Safety
    /// The bytes are readable for `'a`, and nothing writes them while a value
    /// read from them lives, but a writer over them (`Lower::from_raw_parts`) once the
    /// reading is done.
    pub unsafe fn from_raw_parts(at: *const u8, len: usize) -> Lift<'a> {
        Lift {
            at,
            len,
            _pkt: PhantomData,
        }
    }

    /// The packet's bytes, or none for no packet.
    pub fn pkt(&self) -> Option<&'a [u8]> {
        // SAFETY: the reader's bytes (`from_raw_parts`).
        (self.len != 0).then(|| unsafe { core::slice::from_raw_parts(self.at, self.len) })
    }

    /// The `len` bytes at `at`, which the caller checked lie in the packet.
    fn slice(&self, at: usize, len: usize) -> &'a [u8] {
        // SAFETY: the caller's, and the reader's bytes (`from_raw_parts`).
        unsafe { core::slice::from_raw_parts(self.at.add(at), len) }
    }

    fn get<const N: usize>(&self, at: usize) -> Result<[u8; N], Errno> {
        if at.checked_add(N).is_none_or(|end| end > self.len) {
            return Err(Errno::EBADMSG);
        }
        let mut b = [0; N];
        b.copy_from_slice(self.slice(at, N));
        Ok(b)
    }

    /// A bool: 0 is false and any other byte true, as the ABI lifts it.
    pub fn bool(&self, at: usize) -> Result<bool, Errno> {
        Ok(self.get::<1>(at)?[0] != 0)
    }

    pub fn u8(&self, at: usize) -> Result<u8, Errno> {
        self.get(at).map(u8::from_le_bytes)
    }

    pub fn i8(&self, at: usize) -> Result<i8, Errno> {
        self.get(at).map(i8::from_le_bytes)
    }

    pub fn u16(&self, at: usize) -> Result<u16, Errno> {
        self.get(at).map(u16::from_le_bytes)
    }

    pub fn i16(&self, at: usize) -> Result<i16, Errno> {
        self.get(at).map(i16::from_le_bytes)
    }

    pub fn u32(&self, at: usize) -> Result<u32, Errno> {
        self.get(at).map(u32::from_le_bytes)
    }

    pub fn i32(&self, at: usize) -> Result<i32, Errno> {
        self.get(at).map(i32::from_le_bytes)
    }

    pub fn u64(&self, at: usize) -> Result<u64, Errno> {
        self.get(at).map(u64::from_le_bytes)
    }

    pub fn i64(&self, at: usize) -> Result<i64, Errno> {
        self.get(at).map(i64::from_le_bytes)
    }

    pub fn f32(&self, at: usize) -> Result<f32, Errno> {
        self.get(at).map(f32::from_le_bytes)
    }

    pub fn f64(&self, at: usize) -> Result<f64, Errno> {
        self.get(at).map(f64::from_le_bytes)
    }

    /// A discriminant of `n` cases at `at`, its width by `n`, refused where it
    /// names none.
    pub fn case(&self, at: usize, n: usize) -> Result<u32, Errno> {
        let d = match n {
            0..=256 => u32::from(self.u8(at)?),
            257..=65536 => u32::from(self.u16(at)?),
            _ => self.u32(at)?,
        };
        if d as usize >= n {
            Err(Errno::EBADMSG)
        } else {
            Ok(d)
        }
    }

    /// The elements of `sz` bytes at `align` whose offset and count lie at
    /// `at`: where they begin, and how many.
    pub fn list(&self, at: usize, sz: usize, align: usize) -> Result<(usize, usize), Errno> {
        let base = self.u32(at)? as usize;
        let len = self.u32(at + 4)? as usize;
        let fits = len
            .checked_mul(sz)
            .and_then(|n| n.checked_add(base))
            .is_some_and(|end| end <= self.len);
        if !base.is_multiple_of(align) || !fits {
            return Err(Errno::EBADMSG);
        }
        Ok((base, len))
    }

    /// A list of numbers at `at`, copied out.
    pub fn arr<T: Number>(&self, at: usize) -> Result<Vec<T>, Errno> {
        let sz = size_of::<T>();
        let (base, len) = self.list(at, sz, sz)?;
        let mut v = vec![T::default(); len];
        // SAFETY: any bytes are a number's (`Number`), and `v` holds `len * sz`.
        let out = unsafe { core::slice::from_raw_parts_mut(v.as_mut_ptr().cast::<u8>(), len * sz) };
        out.copy_from_slice(self.slice(base, len * sz));
        Ok(v)
    }

    fn bytes(&self, at: usize) -> Result<&'a [u8], Errno> {
        let (base, len) = self.list(at, 1, 1)?;
        Ok(self.slice(base, len))
    }

    pub fn str(&self, at: usize) -> Result<&'a str, Errno> {
        core::str::from_utf8(self.bytes(at)?).map_err(|_| Errno::EBADMSG)
    }
}

#[cfg(test)]
#[path = "cabi.test.rs"]
mod test;
