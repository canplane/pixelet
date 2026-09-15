// xpute_core/kit/text.rs

//! Text with no heap behind it: written into bytes the caller already holds,
//! or kept inline at a fixed capacity. A JavaScript string is the runtime's to
//! allocate; a module whose memory is fixed writes its text into the lanes and
//! the fields it has, and a record holding a string stays plain data.

use core::fmt;

/// A `fmt::Write` over bytes the caller holds: what is written, as UTF-8, up
/// to their length. A write that does not fit writes nothing of itself and
/// errs, so the text is always whole characters.
pub struct Text<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> Text<'a> {
    pub fn new(buf: &'a mut [u8]) -> Text<'a> {
        Text { buf, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: only whole `str`s are ever copied in, end to end.
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl fmt::Write for Text<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// What `T` displays as, written as a JSON string: quoted, with a quote, a
/// backslash and every control character escaped — `JSON.stringify(String(v))`
/// with the control characters other than \n, \r and \t as `\u00XX`.
#[derive(Clone, Copy, Debug)]
pub struct Quoted<T>(pub T);

impl<T: fmt::Display> fmt::Display for Quoted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        f.write_char('"')?;
        let mut escape = Escape(f);
        write!(escape, "{}", self.0)?;
        f.write_char('"')
    }
}

struct Escape<'a, 'b>(&'a mut fmt::Formatter<'b>);

impl fmt::Write for Escape<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut plain = 0;
        for (i, c) in s.char_indices() {
            let escaped = match c {
                '"' => "\\\"",
                '\\' => "\\\\",
                '\n' => "\\n",
                '\r' => "\\r",
                '\t' => "\\t",
                c if (c as u32) < 0x20 => "",
                _ => continue,
            };
            self.0.write_str(&s[plain..i])?;
            if escaped.is_empty() {
                write!(self.0, "\\u{:04x}", c as u32)?;
            } else {
                self.0.write_str(escaped)?;
            }
            plain = i + c.len_utf8();
        }
        self.0.write_str(&s[plain..])
    }
}

/// A string of at most `N` bytes kept inline, so what holds one is plain
/// data. Text past the capacity is cut at the last whole character that fits.
#[derive(Clone, Copy)]
pub struct FixStr<const N: usize> {
    len: usize,
    bytes: [u8; N],
}

impl<const N: usize> FixStr<N> {
    pub const fn new() -> FixStr<N> {
        FixStr { len: 0, bytes: [0; N] }
    }

    /// `s`, cut to the capacity.
    pub fn of(s: &str) -> FixStr<N> {
        let mut out = FixStr::new();
        out.set(s);
        out
    }

    /// Replaces the text with `s`, cut to the capacity.
    pub fn set(&mut self, s: &str) {
        let mut end = s.len().min(N);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.bytes[..end].copy_from_slice(&s.as_bytes()[..end]);
        self.len = end;
    }

    /// Appends `c`; false, and nothing appended, when it does not fit.
    pub fn push(&mut self, c: char) -> bool {
        let mut utf8 = [0; 4];
        fmt::Write::write_str(self, c.encode_utf8(&mut utf8)).is_ok()
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: only whole characters are ever copied in.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

impl<const N: usize> Default for FixStr<N> {
    fn default() -> Self {
        FixStr::new()
    }
}

impl<const N: usize> core::ops::Deref for FixStr<N> {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> PartialEq for FixStr<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> Eq for FixStr<N> {}

impl<const N: usize> PartialEq<str> for FixStr<N> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize> PartialEq<&str> for FixStr<N> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const N: usize> fmt::Debug for FixStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize> fmt::Display for FixStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize> fmt::Write for FixStr<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > N {
            return Err(fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
#[path = "text.test.rs"]
mod test;
