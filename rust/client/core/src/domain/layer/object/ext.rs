// pxl-core/domain/layer/object/ext.rs

//! The records placed things carry beyond their ids, in one pool for each
//! class of record (pxl-spec object/ext.rs). A leaf names its record by the
//! record's place in its class's pool, so a record lives and goes with the
//! leaf that names it. Records are allocated on the heap, whose slab keeps
//! small ones together.

use pxl_spec::object::ext::Class;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ext {
    /// A marker's text, at most `MARKER_TEXT_CHARS` long.
    MarkerText(String),
}

impl Ext {
    pub fn class(&self) -> Class {
        match self {
            Ext::MarkerText(_) => Class::MarkerText,
        }
    }
}

/// The handle of no record. Any other handle is its record's slot plus one.
pub const NONE: u16 = 0;

#[derive(Default)]
struct ClassPool {
    slots: Vec<Option<Ext>>,
    free: Vec<u16>,
}

#[derive(Default)]
pub struct Pool {
    pools: [ClassPool; Class::ALL.len()],
}

impl Pool {
    /// Keeps `ext` in its class's pool and returns its handle. None when the
    /// pool already holds as many records as a handle can name.
    pub fn keep(&mut self, ext: Ext) -> Option<u16> {
        let pool = &mut self.pools[ext.class() as usize];
        if let Some(h) = pool.free.pop() {
            pool.slots[usize::from(h - 1)] = Some(ext);
            return Some(h);
        }
        let h = u16::try_from(pool.slots.len() + 1).ok()?;
        pool.slots.push(Some(ext));
        Some(h)
    }

    pub fn get(&self, class: Class, h: u16) -> Option<&Ext> {
        let slot = usize::from(h.checked_sub(1)?);
        self.pools[class as usize].slots.get(slot)?.as_ref()
    }

    pub fn free(&mut self, class: Class, h: u16) {
        let pool = &mut self.pools[class as usize];
        if let Some(slot) = h
            .checked_sub(1)
            .and_then(|s| pool.slots.get_mut(usize::from(s)))
        {
            if slot.take().is_some() {
                pool.free.push(h);
            }
        }
    }

    #[cfg(test)]
    pub fn clear(&mut self) {
        *self = Pool::default();
    }
}
