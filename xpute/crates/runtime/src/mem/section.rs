// xpute-runtime/mem/section.rs

//! Ranges laid end to end in linear memory, as a linker script places
//! sections at `ALIGN(n)` boundaries. The memory never grows, so a view over
//! it stays valid for the module's life.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub offset: u32,
    pub bytes: u32,
    pub align: u32,
}

impl Section {
    pub const fn end(&self) -> u32 {
        self.offset + self.bytes
    }

    /// The one way into a section, so nothing written through it lands in a
    /// neighbor. None past its end.
    pub const fn at(&self, off: u32, bytes: u32) -> Option<u32> {
        match off.checked_add(bytes) {
            Some(past) if past <= self.bytes => Some(self.offset + off),
            _ => None,
        }
    }

    /// The `n`th of `sizes` laid end to end inside the section on `unit`.
    pub const fn nth(&self, sizes: &[u32], unit: u32, n: usize) -> Section {
        // Const, so a bare panic rather than a bug report.
        if span_of(sizes, unit) > self.bytes {
            panic!()
        }
        Section {
            offset: nth_at(self.offset, sizes, unit, n),
            bytes: sizes[n],
            align: unit,
        }
    }
}

pub const fn align_up(n: u32, unit: u32) -> u32 {
    n.div_ceil(unit) * unit
}

/// Includes the last one's rounding.
pub const fn span_of(sizes: &[u32], unit: u32) -> u32 {
    let (mut span, mut i) = (0, 0);
    while i < sizes.len() {
        span += align_up(sizes[i], unit);
        i += 1;
    }
    span
}

pub const fn nth_at(base: u32, sizes: &[u32], unit: u32, n: usize) -> u32 {
    base + span_of(sizes.split_at(n).0, unit)
}

#[cfg(test)]
#[path = "section.test.rs"]
mod test;
