// pxl-shader/observer.rs

//! What a cast asks of the one it casts for (`Observer`), the eye or the
//! shadow map, and the passes every cast takes, written once over it: each
//! cell's start off the beam rays (`cell`; the beam rays by hand, pxl-render
//! beam.wgsl, light_beam.wgsl), each casting what its
//! observer needs alone (`Observer::needs`). An entry is a pass's call for its
//! observer (beam.rs, cast.rs, raster.rs, light.rs); its lattice, its rays and
//! what its walks leave are its own (eye.rs `Eye`, light.rs `Map`). A pass both
//! observers take is an entry each, its observer fixed by its entry, as
//! Adreno's driver refuses one chosen as it runs
//! (doc/experiment/20261103-shaders-on-two-phones).

use pxl_engine::beam;
use pxl_engine::cast::Detail;

use crate::beam::Words;

/// An observer as the passes ask it: its lattice of beam rays' corners,
/// their cells and its samples, where each one's words lie, and the ray at
/// a point of it. Its rays ask the zooms its own (`detail`), and its beam
/// rays lie apart by a spacing of its own (`Aim::detail`).
pub(crate) trait Observer {
    /// Its beam rays' corners across and down.
    fn corners(&self) -> [u32; 2];
    fn hand_at(&self, idx: u32) -> u32;
    /// Its cells across and down.
    fn cells(&self) -> [u32; 2];
    /// The window of cell `c` its start is the least of: its first corner,
    /// and its side in corners.
    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32);
    /// The zooms corner `c`'s beam ray asks (`corner`), and those cell `c`'s
    /// rays ask (`detail`): a cell reuses its window's beam rays only where
    /// every one asked as its rays do (`cell`).
    fn corner_detail(&self, c: [u32; 2]) -> Detail;
    fn cell_detail(&self, c: [u32; 2]) -> Detail;
    /// The first word of cell `c`'s start (`beam::CELL_WORDS`).
    fn cell_at(&self, c: [u32; 2]) -> u32;
    /// Whether the cast needs cell `c`'s start, off its words `w`: the one
    /// place a pass is told it may pass a thread by.
    fn needs<W: Words + ?Sized>(&self, w: &W, c: [u32; 2]) -> bool;
    /// The cell the thread at `id` of its pass takes, off its words `w`:
    /// none past the lattice's end. The one place a pass is told which
    /// thread takes which.
    fn take<W: Words + ?Sized>(&self, w: &W, id: [u32; 2]) -> [u32; 2];
}

/// The number of `c` on a lattice `across` a row, a corner's.
fn idx(across: u32, c: [u32; 2]) -> u32 {
    c[1] * across + c[0]
}

/// How far the rays of the line of corner `k` start.
fn t_of<O: Observer, W: Words + ?Sized>(o: &O, w: &W, k: u32) -> f32 {
    f32::from_bits(w.word(o.hand_at(k)))
}

/// Where cell `c`'s rays start: the least of the beam rays within its
/// window (`Observer::window`), and that corner, whose line they resume
/// from; and whether they do (`beam::CELL_RESUMES`), not where a beam ray of
/// the window asked other zooms than its rays ask (`cast::Detail::alike`),
/// as the window's least start and line hold only for walks alike.
pub(crate) fn cell<O: Observer, W: Words + ?Sized>(o: &O, w: &mut W, c: [u32; 2]) {
    if !o.needs(&*w, c) {
        return;
    }
    let across = o.corners()[0];
    let ([x, y], side) = o.window(c);
    let asks = o.cell_detail(c);
    let mut from = idx(across, [x, y]);
    let mut least = t_of(o, &*w, from);
    let mut alike = true;
    let mut j = 0;
    while j < side {
        let mut i = 0;
        while i < side {
            let k = idx(across, [x + i, y + j]);
            let t = t_of(o, &*w, k);
            if t < least {
                (least, from) = (t, k);
            }
            alike = alike && o.corner_detail([x + i, y + j]).alike(asks);
            i += 1;
        }
        j += 1;
    }
    let at = o.cell_at(c);
    w.set(at + beam::CELL_T, least.to_bits());
    w.set(at + beam::CELL_CORNER, from);
    w.set(at + beam::CELL_RESUMES, u32::from(alike));
}
