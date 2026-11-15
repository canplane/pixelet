// xpute-kit/codec/json.rs

//! A JSON reader that allocates nothing. `read_json` checks a text against
//! JSON's grammar once; the values it hands out are then read in place, and
//! no tree is built. `Quote` writes a value as a JSON string.

use crate::status::bug::OrBug;
use crate::status::errno::Errno;

/// The grammar check recurses once for each level of nesting, and a
/// module's stack has a fixed size, so a deeper text is refused.
const MAX_DEPTH: u32 = 128;

fn skip_ws(s: &[u8], mut at: usize) -> usize {
    while at < s.len() && matches!(s[at], b' ' | b'\t' | b'\n' | b'\r') {
        at += 1;
    }
    at
}

fn skip_string(s: &[u8], mut at: usize) -> Option<usize> {
    at += 1;
    loop {
        match *s.get(at)? {
            b'"' => return Some(at + 1),
            b'\\' => match *s.get(at + 1)? {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => at += 2,
                b'u' if s.get(at + 2..at + 6)?.iter().all(u8::is_ascii_hexdigit) => at += 6,
                _ => return None,
            },
            b if b < 0x20 => return None,
            // Every byte of a multi-byte UTF-8 sequence is above 0x7F and so
            // is never a quote or a backslash; a string can be walked a byte
            // at a time.
            _ => at += 1,
        }
    }
}

fn skip_val(s: &[u8], at: usize, depth: u32) -> Option<usize> {
    if depth > MAX_DEPTH {
        return None;
    }
    let literal = |word: &[u8]| s[at..].starts_with(word).then_some(at + word.len());
    match *s.get(at)? {
        b'n' => literal(b"null"),
        b't' => literal(b"true"),
        b'f' => literal(b"false"),
        b'"' => skip_string(s, at),
        b'[' => {
            let mut at = skip_ws(s, at + 1);
            if s.get(at) == Some(&b']') {
                return Some(at + 1);
            }
            loop {
                at = skip_ws(s, skip_val(s, at, depth + 1)?);
                match *s.get(at)? {
                    b',' => at = skip_ws(s, at + 1),
                    b']' => return Some(at + 1),
                    _ => return None,
                }
            }
        }
        b'{' => {
            let mut at = skip_ws(s, at + 1);
            if s.get(at) == Some(&b'}') {
                return Some(at + 1);
            }
            loop {
                if s.get(at) != Some(&b'"') {
                    return None;
                }
                at = skip_ws(s, skip_string(s, at)?);
                if s.get(at) != Some(&b':') {
                    return None;
                }
                at = skip_ws(s, skip_val(s, skip_ws(s, at + 1), depth + 1)?);
                match *s.get(at)? {
                    b',' => at = skip_ws(s, at + 1),
                    b'}' => return Some(at + 1),
                    _ => return None,
                }
            }
        }
        _ => skip_number(s, at),
    }
}

/// Follows JSON's number grammar, not the one Rust parses an `f64` by:
/// `+1`, `.5`, `1.` and `01` are refused, as the runtime's `JSON.parse`
/// refuses them.
fn skip_number(s: &[u8], mut at: usize) -> Option<usize> {
    let digits = |at: usize| s[at..].iter().take_while(|c| c.is_ascii_digit()).count();
    if s.get(at) == Some(&b'-') {
        at += 1;
    }
    match digits(at) {
        0 => return None,
        n if n > 1 && s[at] == b'0' => return None,
        n => at += n,
    }
    if s.get(at) == Some(&b'.') {
        match digits(at + 1) {
            0 => return None,
            n => at += 1 + n,
        }
    }
    if matches!(s.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(s.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        match digits(at) {
            0 => return None,
            n => at += n,
        }
    }
    Some(at)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonType {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

#[derive(Clone, Copy)]
pub struct JsonValue<'a> {
    s: &'a str,
}

pub fn read_json(text: &str) -> Option<JsonValue<'_>> {
    let s = text.as_bytes();
    let start = skip_ws(s, 0);
    let end = skip_val(s, start, 0)?;
    (skip_ws(s, end) == s.len()).then(|| JsonValue {
        s: &text[start..end],
    })
}

impl<'a> JsonValue<'a> {
    pub fn type_of(&self) -> JsonType {
        match self.s.as_bytes()[0] {
            b'n' => JsonType::Null,
            b't' | b'f' => JsonType::Bool,
            b'"' => JsonType::String,
            b'[' => JsonType::Array,
            b'{' => JsonType::Object,
            _ => JsonType::Number,
        }
    }

    pub fn is_null(&self) -> bool {
        self.type_of() == JsonType::Null
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        (self.type_of() == JsonType::Number)
            .then(|| self.s.parse().ok())
            .flatten()
    }

    pub fn as_str(&self) -> Option<JsonStr<'a>> {
        (self.type_of() == JsonType::String).then(|| JsonStr {
            raw: &self.s[1..self.s.len() - 1],
        })
    }

    pub fn items(&self) -> JsonItemIter<'a> {
        let open = self.type_of() == JsonType::Array;
        JsonItemIter {
            s: self.s,
            at: if open { 1 } else { self.s.len() },
        }
    }

    /// Every field in the text's order; a key that appears twice is yielded
    /// twice.
    pub fn fields(&self) -> JsonFieldIter<'a> {
        let open = self.type_of() == JsonType::Object;
        JsonFieldIter {
            s: self.s,
            at: if open { 1 } else { self.s.len() },
        }
    }

    /// The last field with that key, which is the one the runtime's
    /// `JSON.parse` keeps.
    pub fn get(&self, key: &str) -> Option<JsonValue<'a>> {
        self.fields()
            .filter(|(k, _)| k.eq_str(key))
            .last()
            .map(|(_, v)| v)
    }

    pub fn at(&self, idx: usize) -> Option<JsonValue<'a>> {
        self.items().nth(idx)
    }
}

impl core::fmt::Debug for JsonValue<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.s)
    }
}

pub struct JsonItemIter<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Iterator for JsonItemIter<'a> {
    type Item = JsonValue<'a>;

    fn next(&mut self) -> Option<JsonValue<'a>> {
        let b = self.s.as_bytes();
        let start = skip_ws(b, self.at);
        if start >= b.len() || b[start] == b']' {
            self.at = b.len();
            return None;
        }
        // `read_json` checked the whole text, so a value starts here, and
        // failing to skip it is a bug.
        let end = skip_val(b, start, 0).or_bug(Errno::ENOTRECOVERABLE);
        self.at = skip_ws(b, end) + 1;
        Some(JsonValue {
            s: &self.s[start..end],
        })
    }
}

pub struct JsonFieldIter<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Iterator for JsonFieldIter<'a> {
    type Item = (JsonStr<'a>, JsonValue<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let b = self.s.as_bytes();
        let key_at = skip_ws(b, self.at);
        if key_at >= b.len() || b[key_at] != b'"' {
            self.at = b.len();
            return None;
        }
        let key_end = skip_string(b, key_at).or_bug(Errno::ENOTRECOVERABLE);
        let val_at = skip_ws(b, skip_ws(b, key_end) + 1);
        let val_end = skip_val(b, val_at, 0).or_bug(Errno::ENOTRECOVERABLE);
        self.at = skip_ws(b, val_end) + 1;
        Some((
            JsonStr {
                raw: &self.s[key_at + 1..key_end - 1],
            },
            JsonValue {
                s: &self.s[val_at..val_end],
            },
        ))
    }
}

/// A JSON string as it stands in the text, without its quotes. Its escapes
/// are decoded only as its characters are read.
#[derive(Clone, Copy)]
pub struct JsonStr<'a> {
    raw: &'a str,
}

impl<'a> JsonStr<'a> {
    pub fn as_plain(&self) -> Option<&'a str> {
        (!self.raw.contains('\\')).then_some(self.raw)
    }

    pub fn chars(&self) -> JsonCharIter<'a> {
        JsonCharIter { rest: self.raw }
    }

    pub fn eq_str(&self, s: &str) -> bool {
        match self.as_plain() {
            Some(plain) => plain == s,
            None => self.chars().eq(s.chars()),
        }
    }
}

impl core::fmt::Display for JsonStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Write;
        match self.as_plain() {
            Some(plain) => f.write_str(plain),
            None => self.chars().try_for_each(|c| f.write_char(c)),
        }
    }
}

impl core::fmt::Debug for JsonStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "\"{}\"", self.raw)
    }
}

/// The characters of a `JsonStr`, its escapes decoded. A `\u` escape of a
/// lone surrogate reads as U+FFFD.
pub struct JsonCharIter<'a> {
    rest: &'a str,
}

fn hex4(s: &str) -> Option<u32> {
    let digits = s.get(..4)?;
    digits
        .bytes()
        .all(|b| b.is_ascii_hexdigit())
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}

impl Iterator for JsonCharIter<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        let mut it = self.rest.chars();
        let c = it.next()?;
        if c != '\\' {
            self.rest = it.as_str();
            return Some(c);
        }
        let e = it.next()?;
        let mut rest = it.as_str();
        let c = match e {
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let hi = hex4(rest)?;
                rest = &rest[4..];
                let mut unit = hi;
                if (0xd800..0xdc00).contains(&hi)
                    && rest.starts_with("\\u")
                    && let Some(lo) = hex4(&rest[2..]).filter(|lo| (0xdc00..0xe000).contains(lo))
                {
                    unit = 0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00);
                    rest = &rest[6..];
                }
                char::from_u32(unit).unwrap_or('\u{fffd}')
            }
            other => other,
        };
        self.rest = rest;
        Some(c)
    }
}

/// Writes a value as `JSON.stringify(String(v))` does: its `Display` text,
/// quoted and escaped.
#[derive(Clone, Copy, Debug)]
pub struct Quote<T>(pub T);

impl<T: core::fmt::Display> core::fmt::Display for Quote<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Write;
        f.write_char('"')?;
        let mut escape = Escape(f);
        write!(escape, "{}", self.0)?;
        f.write_char('"')
    }
}

struct Escape<'a, 'b>(&'a mut core::fmt::Formatter<'b>);

impl core::fmt::Write for Escape<'_, '_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
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

#[cfg(test)]
#[path = "json.test.rs"]
mod test;
