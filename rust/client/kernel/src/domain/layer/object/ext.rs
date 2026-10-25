// pxl-kernel/domain/layer/object/ext.rs

//! What placed things carry beyond their ids, a pool a class
//! (pxl-spec block/ext.rs). A leaf names its record by its place in its
//! class's pool, so a record lives and goes with the leaf that names it.
//! A record's memory is the heap's, whose slab keeps small ones together.

use pxl_spec::block::ext::Class;

/// One record, in its class's shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ext {
    /// A marker's words, within `MARKER_TEXT_CHARS`.
    MarkerText(String),
}

impl Ext {
    pub fn class(&self) -> Class {
        match self {
            Ext::MarkerText(_) => Class::marker_text,
        }
    }
}

/// The handle a leaf holds: none, or one past its record's slot.
pub const NONE: u16 = 0;

#[derive(Default)]
struct Pool {
    slots: Vec<Option<Ext>>,
    free: Vec<u16>,
}

#[derive(Default)]
pub struct Pools {
    pools: [Pool; Class::ALL.len()],
}

impl Pools {
    /// Keeps `ext` in its class's pool; none where the pool holds as many
    /// records as a handle can name.
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

    pub fn clear(&mut self) {
        *self = Pools::default();
    }
}
