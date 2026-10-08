// pxl-kernel/layer/object/preview.rs

//! An edit not yet made, as what it changes in what is drawn: the pointer
//! decides it, the chunks draw without it; and a marker waiting for its
//! words, drawn with the words it has so far.

use pxl_spec::bake::constant::EntryAt;
use crate::sched::edge::request_world_tick;
use xpute_guest::global::Global;

/// What the eraser's preview changes in the drawn build only: the pick and
/// the cell queries still see what it hides, or hover would clear, the hide
/// lift and the pick hit it again, forever. No revision moves, so saves do
/// not see the hover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderEdits {
    /// The placed object hidden, and whether it is drawn in tiers, decor, or
    /// (already gone) either.
    pub hide: Option<EntryAt>,
    pub hide_in: (bool, bool),
    /// An AIR entry over bare terrain, which has no leaf to hide.
    pub carve: Option<EntryAt>,
    /// The scattered decor hidden, by the cell it stands in.
    pub scattered: Option<[i64; 3]>,
}

/// A marker the tool left waiting for its words, at a place free or holding
/// a marker: put up there with them, it is the marker that stands there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub at: EntryAt,
    pub words: String,
}

static DRAFT: Global<Option<Draft>> = Global::new();

/// The words a marker at `at` shows: the waiting one's so far, else those
/// put up there; none where no marker stands or waits.
pub fn words_at(at: EntryAt) -> Option<&'static str> {
    if let Some(d) = draft().filter(|d| d.at == at) {
        return Some(&d.words);
    }
    let objects = &crate::kernel().objects;
    objects.words(&objects.placed(at.zoom(), at.at())?)
}

/// The one waiting, which the badges draw.
pub fn draft() -> Option<&'static Draft> {
    DRAFT.get(|| None).as_ref()
}

/// The one waiting, for the tool to change, which the next world turn draws.
pub fn redraft() -> &'static mut Option<Draft> {
    request_world_tick();
    DRAFT.get(|| None)
}
