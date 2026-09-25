// xpute-core/codec/json.rs

//! `JSON.stringify` and `JSON.parse` of a flat string map — the one JSON
//! the records carry (a POI's tags, a placement's), written by either side
//! and read by the other, so the text is the host's to the byte.
//!
//! What the host does that a naive port would not:
//! - an object's own key order puts array-index keys ("0", "12") first in
//!   ascending number, then the rest in insertion order, and both
//!   stringify and parse follow it;
//! - a key given twice keeps its first position and its last value;
//! - an `undefined` value is left out of the text;
//! - a string escapes `"`, `\`, the five short controls, and every other
//!   code unit below U+0020 as `\u00xx` in lowercase hex, and nothing else.
//!
//! The one thing it cannot be: a lone surrogate, which a JavaScript string
//! holds and a Rust one cannot; `\ud800` alone parses to U+FFFD.

use crate::status::bug::OrBug;
use crate::status::errno::Errno;
use crate::status::error::MarshalError;

/// `Record<string, string | undefined>`, in the order it was built.
pub type StrMap = Vec<(String, Option<String>)>;

/// A canonical array index: "0", or digits with no leading zero, below
/// 2^32 − 1.
fn is_array_index(k: &str) -> bool {
    if k.is_empty() || k.len() > 10 || !k.bytes().all(|b| b.is_ascii_digit()) || (k.len() > 1 && k.starts_with('0')) {
        return false;
    }
    k.parse::<u64>().is_ok_and(|n| n < 0xffff_ffff)
}

/// The entries as the object holds them: a key once, at its first
/// position with its last value, index keys first.
fn ordered(map: &[(String, Option<String>)]) -> Vec<(&str, Option<&str>)> {
    let mut out: Vec<(&str, Option<&str>)> = Vec::with_capacity(map.len());
    for (k, v) in map {
        match out.iter_mut().find(|(have, _)| *have == k.as_str()) {
            Some(slot) => slot.1 = v.as_deref(),
            None => out.push((k.as_str(), v.as_deref())),
        }
    }
    let (mut index, rest): (Vec<_>, Vec<_>) = out.into_iter().partition(|(k, _)| is_array_index(k));
    index.sort_by_key(|(k, _)| k.parse::<u64>().unwrap());
    index.extend(rest);
    index
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `JSON.stringify(map)`.
pub fn stringify_str_map(map: &[(String, Option<String>)]) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in ordered(map) {
        let Some(v) = v else { continue };
        if !first {
            out.push(',');
        }
        first = false;
        quote(&mut out, k);
        out.push(':');
        quote(&mut out, v);
    }
    out.push('}');
    out
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn fail(&self) -> MarshalError {
        MarshalError::new(Errno::EBADMSG)
    }

    fn ws(&mut self) {
        while self.at < self.s.len() && matches!(self.s[self.at], b' ' | b'\t' | b'\n' | b'\r') {
            self.at += 1;
        }
    }

    fn eat(&mut self, b: u8) -> Result<(), MarshalError> {
        self.ws();
        if self.s.get(self.at) != Some(&b) {
            return Err(self.fail());
        }
        self.at += 1;
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, MarshalError> {
        let digits = self.s.get(self.at..self.at + 4).ok_or_else(|| self.fail())?;
        let text = core::str::from_utf8(digits).map_err(|_| self.fail())?;
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(self.fail());
        }
        self.at += 4;
        Ok(u32::from_str_radix(text, 16).unwrap())
    }

    fn string(&mut self) -> Result<String, MarshalError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&b) = self.s.get(self.at) else { return Err(self.fail()) };
            match b {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let Some(&e) = self.s.get(self.at) else { return Err(self.fail()) };
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let unit = if (0xd800..0xdc00).contains(&hi) && self.s.get(self.at..self.at + 2) == Some(b"\\u") {
                                let save = self.at;
                                self.at += 2;
                                let lo = self.hex4()?;
                                if (0xdc00..0xe000).contains(&lo) {
                                    0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                                } else {
                                    self.at = save;
                                    hi
                                }
                            } else {
                                hi
                            };
                            out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.fail()),
                    }
                }
                b if b < 0x20 => return Err(self.fail()),
                _ => {
                    // A whole UTF-8 sequence: the input is a &str, so it is valid.
                    let len = match b {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    out.push_str(core::str::from_utf8(&self.s[self.at..self.at + len]).unwrap());
                    self.at += len;
                }
            }
        }
    }
}

/// `JSON.parse(text)` for an object whose values are strings. Anything
/// else — another value type, trailing text — is refused, where the host
/// would hand back whatever the text held.
pub fn parse_str_map(text: &str) -> Result<StrMap, MarshalError> {
    let mut p = Parser { s: text.as_bytes(), at: 0 };
    p.eat(b'{')?;
    let mut raw: StrMap = Vec::new();
    p.ws();
    if p.s.get(p.at) == Some(&b'}') {
        p.at += 1;
    } else {
        loop {
            let k = p.string()?;
            p.eat(b':')?;
            p.ws();
            let v = p.string()?;
            raw.push((k, Some(v)));
            p.ws();
            match p.s.get(p.at) {
                Some(b',') => p.at += 1,
                Some(b'}') => {
                    p.at += 1;
                    break;
                }
                _ => return Err(p.fail()),
            }
        }
    }
    p.ws();
    if p.at != p.s.len() {
        return Err(p.fail());
    }
    Ok(ordered(&raw).into_iter().map(|(k, v)| (k.to_string(), v.map(str::to_string))).collect())
}

// ============ Reading in place ============
//
// `JSON.parse` for a module with no heap: the text is checked once, and what
// is read of it is read where it lies — a value is the span of its bytes, an
// array or an object is walked when asked, and a string is decoded character
// by character as it is compared or written out. Nothing is built.

/// How deep arrays and objects may nest before a text is refused: the check
/// recurses, and a module's stack is fixed.
const MAX_DEPTH: u32 = 128;

fn skip_ws(s: &[u8], mut at: usize) -> usize {
    while at < s.len() && matches!(s[at], b' ' | b'\t' | b'\n' | b'\r') {
        at += 1;
    }
    at
}

/// Where the string opening at `at` ends, past its closing quote.
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
            // A byte of a UTF-8 sequence is never a quote or a backslash, so
            // the text is walked a byte at a time.
            _ => at += 1,
        }
    }
}

/// Where the value starting at `at` ends.
fn skip_value(s: &[u8], at: usize, depth: u32) -> Option<usize> {
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
                at = skip_ws(s, skip_value(s, at, depth + 1)?);
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
                at = skip_ws(s, skip_value(s, skip_ws(s, at + 1), depth + 1)?);
                match *s.get(at)? {
                    b',' => at = skip_ws(s, at + 1),
                    b'}' => return Some(at + 1),
                    _ => return None,
                }
            }
        }
        _ => {
            let mut end = at;
            while end < s.len() && matches!(s[end], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                end += 1;
            }
            core::str::from_utf8(&s[at..end]).ok()?.parse::<f64>().ok().map(|_| end)
        }
    }
}

/// What a value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonKind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

/// One JSON value where it lies in the text it was read from.
#[derive(Clone, Copy)]
pub struct JsonValue<'a> {
    /// The value's own bytes, checked, from its first to its last.
    s: &'a str,
}

/// `JSON.parse(text)` in place: the one value the text holds, or none where
/// it holds anything else.
pub fn read_json(text: &str) -> Option<JsonValue<'_>> {
    let s = text.as_bytes();
    let start = skip_ws(s, 0);
    let end = skip_value(s, start, 0)?;
    (skip_ws(s, end) == s.len()).then(|| JsonValue { s: &text[start..end] })
}

impl<'a> JsonValue<'a> {
    pub fn kind(&self) -> JsonKind {
        match self.s.as_bytes()[0] {
            b'n' => JsonKind::Null,
            b't' | b'f' => JsonKind::Bool,
            b'"' => JsonKind::String,
            b'[' => JsonKind::Array,
            b'{' => JsonKind::Object,
            _ => JsonKind::Number,
        }
    }

    pub fn is_null(&self) -> bool {
        self.kind() == JsonKind::Null
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        (self.kind() == JsonKind::Number).then(|| self.s.parse().ok()).flatten()
    }

    pub fn as_str(&self) -> Option<JsonStr<'a>> {
        (self.kind() == JsonKind::String).then(|| JsonStr { raw: &self.s[1..self.s.len() - 1] })
    }

    /// An array's items in order; nothing for any other value.
    pub fn items(&self) -> JsonItems<'a> {
        let open = self.kind() == JsonKind::Array;
        JsonItems {
            s: self.s,
            at: if open { 1 } else { self.s.len() },
        }
    }

    /// An object's fields in order, a key given twice given twice; nothing
    /// for any other value.
    pub fn fields(&self) -> JsonFields<'a> {
        let open = self.kind() == JsonKind::Object;
        JsonFields {
            s: self.s,
            at: if open { 1 } else { self.s.len() },
        }
    }

    /// `value[key]` on an object: the last field of that name, as the host's
    /// parse keeps.
    pub fn get(&self, key: &str) -> Option<JsonValue<'a>> {
        self.fields().filter(|(k, _)| k.eq_str(key)).last().map(|(_, v)| v)
    }

    /// `value[index]` on an array.
    pub fn at(&self, index: usize) -> Option<JsonValue<'a>> {
        self.items().nth(index)
    }
}

impl core::fmt::Debug for JsonValue<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.s)
    }
}

pub struct JsonItems<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Iterator for JsonItems<'a> {
    type Item = JsonValue<'a>;

    fn next(&mut self) -> Option<JsonValue<'a>> {
        let b = self.s.as_bytes();
        let start = skip_ws(b, self.at);
        if start >= b.len() || b[start] == b']' {
            self.at = b.len();
            return None;
        }
        // The text was checked whole, so a value is where the grammar says.
        let end = skip_value(b, start, 0).or_bug(Errno::ENOTRECOVERABLE);
        self.at = skip_ws(b, end) + 1;
        Some(JsonValue { s: &self.s[start..end] })
    }
}

pub struct JsonFields<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Iterator for JsonFields<'a> {
    type Item = (JsonStr<'a>, JsonValue<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let b = self.s.as_bytes();
        let key_at = skip_ws(b, self.at);
        if key_at >= b.len() || b[key_at] != b'"' {
            self.at = b.len();
            return None;
        }
        let key_end = skip_string(b, key_at).or_bug(Errno::ENOTRECOVERABLE);
        let value_at = skip_ws(b, skip_ws(b, key_end) + 1);
        let value_end = skip_value(b, value_at, 0).or_bug(Errno::ENOTRECOVERABLE);
        self.at = skip_ws(b, value_end) + 1;
        Some((
            JsonStr {
                raw: &self.s[key_at + 1..key_end - 1],
            },
            JsonValue { s: &self.s[value_at..value_end] },
        ))
    }
}

/// A JSON string where it lies: its characters between the quotes, escapes
/// as written, decoded as they are read.
#[derive(Clone, Copy)]
pub struct JsonStr<'a> {
    raw: &'a str,
}

impl<'a> JsonStr<'a> {
    /// The string itself, where it holds no escape: most do.
    pub fn as_plain(&self) -> Option<&'a str> {
        (!self.raw.contains('\\')).then_some(self.raw)
    }

    pub fn chars(&self) -> JsonChars<'a> {
        JsonChars { rest: self.raw }
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

/// A JSON string's characters, escapes decoded; a lone surrogate reads as U+FFFD.
pub struct JsonChars<'a> {
    rest: &'a str,
}

fn hex4(s: &str) -> Option<u32> {
    let digits = s.get(..4)?;
    digits.bytes().all(|b| b.is_ascii_hexdigit()).then(|| u32::from_str_radix(digits, 16).ok()).flatten()
}

impl Iterator for JsonChars<'_> {
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
                if (0xd800..0xdc00).contains(&hi) && rest.starts_with("\\u") {
                    if let Some(lo) = hex4(&rest[2..]).filter(|lo| (0xdc00..0xe000).contains(lo)) {
                        unit = 0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00);
                        rest = &rest[6..];
                    }
                }
                char::from_u32(unit).unwrap_or('\u{fffd}')
            }
            other => other,
        };
        self.rest = rest;
        Some(c)
    }
}

// ============ Writing in place ============

/// What `T` displays as, written as a JSON string: quoted, with a quote, a
/// backslash and every control character escaped — `JSON.stringify(String(v))`
/// with the control characters other than \n, \r and \t as `\u00XX`.
#[derive(Clone, Copy, Debug)]
pub struct Quoted<T>(pub T);

impl<T: core::fmt::Display> core::fmt::Display for Quoted<T> {
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
