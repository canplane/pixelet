// pxl-shader/observer.rs

//! What a cast asks of the one it casts for (`Observer`), the eye or the
//! shadow map, and the passes every cast takes, written once over it: each
//! corner's beam ray (`beam`), each cell's start off them (`cell`), each
//! sample's walk (`sample`) and box (`sample_box`), a ray's visible sample
//! (`took`), the sky about it (`sky`) or a walk of its own (`resolve`), and its
//! walk resumed off the beam (`resume`, `walked`), each casting what its
//! observer needs alone (`Observer::needs`). An entry is a pass's call for its
//! observer (beam.rs, cast.rs, raster.rs, sun.rs); its lattice, its rays and
//! what its walks leave are its own (eye.rs `Eye`, sun.rs `Map`). A pass both
//! observers take is one entry, its observer the one its head names
//! (either.rs `Either`), so a pass is one module whichever casts it.

use pxl_spec_core::cast::{self, Detail, Met, Query, Until};
use pxl_spec_core::{beam, observed, raster, tally};

use crate::beam::{resumed, sample_put, Beamed, Leaving, Words};
use crate::observed::{add_tally, seen_at, Seen, Writ, Writing};
use crate::raster::{boxed, taken, NONE};
use crate::world::{Looks, World};

/// A ray as an observer aims it: where it starts, its way and reach, and
/// the zooms it asks with the spacing its beam rays lie apart by
/// (`cast::Detail`).
#[derive(Clone, Copy)]
pub(crate) struct Aim {
    pub(crate) origin: [i32; 3],
    pub(crate) ray: cast::Ray,
    pub(crate) detail: Detail,
}

/// The levels of an observer's lattice a pass casts (`Observer::needs`): a
/// corner's beam ray, a cell's start, a sample, a ray at a point.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum Level {
    Corner,
    Cell,
    Sample,
    Ray,
}

/// An observer as the passes ask it: its lattice of beam rays' corners,
/// their cells and its samples, where each one's words lie, and the ray at
/// a point of it. Its rays ask the zooms the eye asks at each point, and
/// its beam rays lie apart by a spacing of its own (`Aim::detail`).
pub(crate) trait Observer {
    /// Where on its lattice a ray is aimed: the eye's tangents across and
    /// up, the map's slot.
    type At: Copy;
    /// Its run of lanes in the tally (pxl-spec-core `tally::EYE`,
    /// `tally::SUN`).
    fn tally(&self) -> u32;
    /// Whether its walks write the cells a ray sees into the observed
    /// pyramid (observed.rs `Writing::assets`).
    fn assets(&self) -> bool;
    /// The passes its beam, its samples and its walks again write the
    /// observed pyramid under (pxl-spec-core `observed::BEAM_PASS`).
    fn passes(&self) -> [u32; 3];
    /// Whether its rays start apart, a line's nodes held from its own
    /// origin (`origin_of`); else every ray starts at one, a line's node
    /// its own.
    fn apart(&self) -> bool;

    /// Its beam rays' corners across and down.
    fn corners(&self) -> [u32; 2];
    /// Corner `c`'s beam ray.
    fn corner(&self, c: [u32; 2]) -> Aim;
    /// How far the rays about a beam ray aimed `aim` start, where it
    /// stopped at `t`: no farther than any of theirs enters what stopped
    /// it.
    fn handed(&self, aim: &Aim, t: f32) -> f32;
    /// The first word of the line of the corner numbered `index`, `across` a
    /// row (`index`).
    fn line(&self, index: u32) -> u32;
    /// Where the ray of the line at `base` of `w` started, which its nodes
    /// are held from (`cast::Node::moved`); and that kept there.
    fn origin_of<W: Words + ?Sized>(&self, w: &W, base: u32) -> [i32; 3];
    fn keep_origin<W: Words + ?Sized>(&self, w: &mut W, base: u32, o: [i32; 3]);
    /// Its cells across and down.
    fn cells(&self) -> [u32; 2];
    /// The window of cell `c` its start is the least of: its first corner,
    /// and its side in corners.
    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32);
    /// The first word of cell `c`'s start (`beam::CELL_WORDS`).
    fn cell_at(&self, c: [u32; 2]) -> u32;
    /// Its samples across and down.
    fn samples(&self) -> [u32; 2];
    /// Where sample `s` is aimed, the cell it starts from, and the first
    /// word of its stop (`beam::SAMPLE_WORDS`).
    fn point(&self, s: [u32; 2]) -> Self::At;
    fn sample_cell(&self, s: [u32; 2]) -> [u32; 2];
    fn sample_at(&self, s: [u32; 2]) -> u32;
    /// The first of the four samples about `at`, the others the next
    /// across, down and both, and whether the four lie about it in the
    /// world, as none does across an edge its lattice wraps at.
    fn sample_before(&self, at: Self::At) -> ([u32; 2], bool);
    /// The ray at `at`, and the cell it starts from.
    fn aim(&self, at: Self::At) -> Aim;
    fn cell_of(&self, at: Self::At) -> [u32; 2];
    /// From where its boxes are laid for the raster, and whether a box
    /// about the eye is marked and at the screen's tangents (raster.rs
    /// `boxed`).
    fn laid(&self) -> ([i32; 3], (bool, [f32; 2]));
    /// Whether the cast needs what `level` at `at` of its lattice gives,
    /// off its words `w`: the one place a pass is told it may pass a thread
    /// by.
    fn needs<W: Words + ?Sized>(&self, w: &W, level: Level, at: [u32; 2]) -> bool;
}

/// The number of `c` on a lattice `across` a row, a corner's or a sample's.
fn index(across: u32, c: [u32; 2]) -> u32 {
    c[1] * across + c[0]
}

/// Corner `c`'s beam ray walked from the world's root to the first cell
/// that fills or lacks, or the first split too narrow to share
/// (`cast::Until::Beam`), through the observed pyramid writing what it asks,
/// its line left in `w` for the rays about it (`Leaving`), how far they
/// start from it and the nodes it visited.
pub(crate) fn beam<O: Observer, W: Words + ?Sized, L: Looks>(
    o: &O,
    world: World<'_, L>,
    seen: &mut Seen,
    w: &mut W,
    c: [u32; 2],
) {
    if !o.needs(&*w, Level::Corner, c) {
        return;
    }
    let aim = o.corner(c);
    let base = o.line(index(o.corners()[0], c));
    o.keep_origin(w, base, aim.origin);
    let tag = observed::tag(seen_at(seen, observed::FRAME), o.passes()[0]);
    let met = cast::rooted(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            assets: o.assets(),
        },
        aim.origin,
        &aim.ray,
        aim.detail,
        Until::Beam,
        &mut Leaving(&mut *w, base),
    );
    add_tally(seen, o.tally(), tally::BEAM, met.visits);
    w.set(base + beam::LINE_T, o.handed(&aim, met.stop.t).to_bits());
    w.set(base + beam::LINE_VISITS, met.visits);
}

/// How far the rays of the line of corner `k` start.
fn t_of<O: Observer, W: Words + ?Sized>(o: &O, w: &W, k: u32) -> f32 {
    f32::from_bits(w.word(o.line(k) + beam::LINE_T))
}

/// Where cell `c`'s rays start: the least of the beam rays within its
/// window (`Observer::window`), and that corner, whose line they resume
/// from.
pub(crate) fn cell<O: Observer, W: Words + ?Sized>(o: &O, w: &mut W, c: [u32; 2]) {
    if !o.needs(&*w, Level::Cell, c) {
        return;
    }
    let across = o.corners()[0];
    let ([x, y], side) = o.window(c);
    let mut from = index(across, [x, y]);
    let mut least = t_of(o, &*w, from);
    let mut j = 0;
    while j < side {
        let mut i = 0;
        while i < side {
            let k = index(across, [x + i, y + j]);
            let t = t_of(o, &*w, k);
            if t < least {
                (least, from) = (t, k);
            }
            i += 1;
        }
        j += 1;
    }
    let at = o.cell_at(c);
    w.set(at + beam::CELL_T, least.to_bits());
    w.set(at + beam::CELL_CORNER, from);
}

/// The walk of the ray aimed `aim` to the cell that fills, resumed at
/// cell `cell`'s start from the deepest node of its line holding where it
/// is there, held from the ray's own origin, or from the root at it where
/// that lies in another root (beam.rs `resumed`); through `q`, reading or
/// writing, its line left where it ends to `keep`. Its met, and the nodes
/// the line's beam ray visited.
pub(crate) fn resume<'a, O: Observer, Q: Query, W: Words + ?Sized>(
    o: &O,
    q: &mut Q,
    w: &'a W,
    aim: &Aim,
    cell: [u32; 2],
    keep: &mut impl cast::Keep<Q::At>,
) -> (Met<Q::At>, u32)
where
    Beamed<'a, W>: cast::Over<Q::At>,
{
    let at = o.cell_at(cell);
    let t = f32::from_bits(w.word(at + beam::CELL_T));
    let line = Beamed(w, o.line(w.word(at + beam::CELL_CORNER)));
    let node = if o.apart() {
        line.node().moved(o.origin_of(w, line.1), aim.origin)
    } else {
        line.node()
    };
    let met = resumed(q, aim.origin, &aim.ray, aim.detail, t, &line, node, keep);
    (met, line.visits())
}

/// Sample `s`'s ray resumed from its cell's start, walked to the cell that
/// fills through the observed pyramid writing what it asks, and its stop
/// left in `w` for the raster (`sample_put`), with the nodes it and its
/// beam ray visited.
pub(crate) fn sample<O: Observer, W: Words + ?Sized, L: Looks>(
    o: &O,
    world: World<'_, L>,
    seen: &mut Seen,
    w: &mut W,
    s: [u32; 2],
) {
    // One not needed meets nothing, so draws no box (`sample_box`).
    if !o.needs(&*w, Level::Sample, s) {
        w.set(
            o.sample_at(s) + beam::SAMPLE_NODE + beam::NODE_ZOOM,
            beam::SAMPLE_NONE,
        );
        return;
    }
    let aim = o.aim(o.point(s));
    let tag = observed::tag(seen_at(seen, observed::FRAME), o.passes()[1]);
    let (met, before) = resume(
        o,
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            assets: o.assets(),
        },
        &*w,
        &aim,
        o.sample_cell(s),
        &mut (),
    );
    add_tally(seen, o.tally(), tally::SAMPLE, met.visits);
    let met = Met {
        visits: met.visits + before,
        ..met
    };
    sample_put(w, o.sample_at(s), &met);
}

/// Sample `s`'s stop written as a box for the raster's draw (raster.rs
/// `boxed`), the one across and up before it: its place among the boxes,
/// `NONE` where it wrote none.
pub(crate) fn sample_box<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    s: [u32; 2],
    boxes: &mut [u32],
    args: &mut [u32],
) -> u32 {
    let before = [
        if s[0] > 0 {
            o.sample_at([s[0] - 1, s[1]])
        } else {
            NONE
        },
        if s[1] > 0 {
            o.sample_at([s[0], s[1] - 1])
        } else {
            NONE
        },
    ];
    let (origin, eyed) = o.laid();
    boxed(
        w,
        o.sample_at(s),
        before,
        origin,
        index(o.samples()[0], s),
        eyed,
        boxes,
        args,
    )
}

/// What a ray at `at` aimed `aim` takes (`resolve`): the cell off the
/// visible sample, the sky, or a walk of its own.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum Take {
    Sample,
    Sky,
    Walk,
}

/// What the ray at `at` aimed `aim` takes, `visible` the visibility buffer's
/// there: the cell it enters as the visible sample entered it (`took`), its
/// stop and that sample's first word; else the sky where no sample about it met
/// anything and none was visible (`sky`); else a walk of its own, which its
/// pass lists for the walk again. The eye's pixels and the sun's slots alike.
pub(crate) fn resolve<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    at: O::At,
    aim: &Aim,
    visible: u32,
) -> (Take, cast::Stop<u32>, u32) {
    if visible != 0 && visible & raster::LACK == 0 {
        let (stop, k, enters) = took(o, w, aim, visible);
        if enters {
            return (Take::Sample, stop, k);
        }
    }
    let none = cast::Stop {
        t: aim.ray.reach,
        axis: 0,
        node: cast::Node::ROOT,
        at: 0u32,
        asset: [0; 2],
    };
    if visible == 0 && sky(o, w, at) {
        (Take::Sky, none, 0)
    } else {
        (Take::Walk, none, 0)
    }
}

/// The cell the ray aimed `aim` takes off the visible sample `visible`, a
/// sample's number that lacks not (`raster::sample_of`): where it enters it
/// as that sample entered it (raster.rs `taken`), the sample's first word,
/// and whether it does.
pub(crate) fn took<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    aim: &Aim,
    visible: u32,
) -> (cast::Stop<u32>, u32, bool) {
    let (n, s) = (o.samples()[0], raster::sample_of(visible));
    let k = o.sample_at([s % n, s / n]);
    let (stop, enters) = taken(w, k, aim.origin, &aim.ray);
    (stop, k, enters)
}

/// Whether the four samples about `at` each went on to its reach
/// (`beam::SAMPLE_NONE`), meeting nothing: the ray there takes the sky
/// without a walk of its own, as a thing thinner than the samples' spacing
/// between them goes unseen. None where the four lie not about it.
pub(crate) fn sky<O: Observer, W: Words + ?Sized>(o: &O, w: &W, at: O::At) -> bool {
    let ([i, j], about) = o.sample_before(at);
    about
        && went_on(o, w, [i, j])
        && went_on(o, w, [i + 1, j])
        && went_on(o, w, [i, j + 1])
        && went_on(o, w, [i + 1, j + 1])
}

/// Whether sample `s` went on to its reach (`sky`). [rust-gpu] A call of
/// its own, not a closure over `w`, whose capture spirv-val refuses as no
/// logical pointer.
fn went_on<O: Observer, W: Words + ?Sized>(o: &O, w: &W, s: [u32; 2]) -> bool {
    w.word(o.sample_at(s) + beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE
}

/// The ray at `at` walked again from its cell's start through the observed
/// pyramid writing what it asks, a ray its visible sample and the pyramid left
/// unmet, its line left where it ends to `keep`: its aim, its met and the
/// nodes its beam ray visited.
pub(crate) fn walked<O: Observer, W: Words + ?Sized, L: Looks>(
    o: &O,
    world: World<'_, L>,
    seen: &mut Seen,
    w: &W,
    at: O::At,
    keep: &mut impl cast::Keep<Writ>,
) -> (Aim, Met<Writ>, u32) {
    let aim = o.aim(at);
    let tag = observed::tag(seen_at(seen, observed::FRAME), o.passes()[2]);
    let (met, before) = resume(
        o,
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            assets: o.assets(),
        },
        w,
        &aim,
        o.cell_of(at),
        keep,
    );
    add_tally(seen, o.tally(), tally::MISS, met.visits);
    (aim, met, before)
}
