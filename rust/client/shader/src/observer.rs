// pxl-shader/observer.rs

//! What a cast asks of the one it casts for (`Observer`), the eye or the
//! shadow map, and the passes every cast takes, written once over it: each
//! corner's beam ray (`beam`), each cell's start off them (`cell`), each
//! sample's box (`sample_box`; its walk by hand, pxl-render sample.wgsl), a ray's visible sample
//! (`took`), the sky about it (`sky`) or a walk of its own (`resolve`), and its
//! walk resumed off the beam (`resume`, `walk_again`), each casting what its
//! observer needs alone (`Observer::needs`). An entry is a pass's call for its
//! observer (beam.rs, cast.rs, raster.rs, light.rs); its lattice, its rays and
//! what its walks leave are its own (eye.rs `Eye`, light.rs `Map`). A pass both
//! observers take is an entry each, its observer fixed by its entry, as
//! Adreno's driver refuses one chosen as it runs
//! (doc/experiment/20261103-shaders-on-two-phones).

use pxl_engine::cast::{self, Detail, Trace, Query, Until};
use pxl_engine::world::Inheritance;
use pxl_engine::{beam, pyramid, raster, tally};

use crate::beam::{resume_walk, BeamLine, Leaving, Words};
use crate::pyramid::{add_tally, pyramid_at, Pyramid, Writing};
use crate::raster::{write_box, visible_cell, NONE};
use crate::world::{PaletteWords, World};

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
/// a point of it. Its rays ask the zooms its own (`detail`), and its beam
/// rays lie apart by a spacing of its own (`Aim::detail`).
pub(crate) trait Observer {
    /// Where on its lattice a ray is aimed: the eye's tangents across and
    /// up, the map's slot.
    type At: Copy;
    /// Its run of lanes in the tally (pxl-engine `tally::EYE`,
    /// `tally::LIGHT`).
    fn tally(&self) -> u32;
    /// Whether its walks write the cells a ray sees into the observed
    /// pyramid (pyramid.rs `Writing::assets`).
    fn assets(&self) -> bool;
    /// The passes its beam, its samples and its walks again write the
    /// observed pyramid under (pxl-engine `pyramid::BEAM_PASS`).
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
    /// row (`index`), and the word of how far the rays about it start
    /// (`handed`), a word a corner in their order apart from the lines.
    fn line(&self, idx: u32) -> u32;
    fn hand_at(&self, idx: u32) -> u32;
    /// Where the ray of the line at `base` of `w` started, which its nodes
    /// are held from (`cast::Node::moved`); and that kept there.
    fn origin_of<W: Words + ?Sized>(&self, w: &W, base: u32) -> [i32; 3];
    fn keep_origin<W: Words + ?Sized>(&self, w: &mut W, base: u32, o: [i32; 3]);
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
    /// Its samples across and down.
    fn samples(&self) -> [u32; 2];
    /// Where sample `s` is aimed, and the first word of its hit
    /// (`beam::SAMPLE_WORDS`).
    fn point(&self, s: [u32; 2]) -> Self::At;
    fn sample_at(&self, s: [u32; 2]) -> u32;
    /// The first of the four samples about `at`, the others the next
    /// across, down and both, and whether the four lie about it in the
    /// world, as none does across an edge its lattice wraps at.
    fn sample_before(&self, at: Self::At) -> ([u32; 2], bool);
    /// The ray at `at`, and the cell it starts from.
    fn aim(&self, at: Self::At) -> Aim;
    fn cell_of(&self, at: Self::At) -> [u32; 2];
    /// The zooms the ray at `at` asks (`aim`): the eye's, one everywhere;
    /// the map's, its tile's band (light.rs `Map`). A ray takes another's
    /// cell or sky only where that one asked as it does (`took`, `sky`).
    fn detail(&self, at: Self::At) -> Detail;
    /// From where its boxes are laid for the raster, and whether a box
    /// about the eye is marked and at the screen's tangents (raster.rs
    /// `write_box`).
    fn raster_layout(&self) -> ([i32; 3], (bool, [f32; 2]));
    /// Whether the cast needs what `level` at `at` of its lattice gives,
    /// off its words `w`: the one place a pass is told it may pass a thread
    /// by.
    fn needs<W: Words + ?Sized>(&self, w: &W, level: Level, at: [u32; 2]) -> bool;
    /// What of `level` of its lattice the thread at `id` of its pass takes,
    /// off its words `w`: none past the lattice's end. The one place a pass
    /// is told which thread takes which.
    fn take<W: Words + ?Sized>(&self, w: &W, level: Level, id: [u32; 2]) -> [u32; 2];
    /// The corner the thread at `id` of `beam_cs` walks: none past those it has.
    fn corner_walked<W: Words + ?Sized>(&self, w: &W, id: [u32; 2]) -> [u32; 2] {
        self.take(w, Level::Corner, id)
    }
}

/// The number of `c` on a lattice `across` a row, a corner's or a sample's.
fn idx(across: u32, c: [u32; 2]) -> u32 {
    c[1] * across + c[0]
}

/// Corner `c`'s beam ray walked from the world's root to the first cell
/// that fills or lacks, or the first split too narrow to share
/// (`cast::Until::Beam`), through the observed pyramid writing what it asks,
/// its line left in `w` for the rays about it (`Leaving`), how far they
/// start from it and the nodes it visited; none where the cast needs it not.
pub(crate) fn beam<O: Observer, W: Words + ?Sized, L: PaletteWords>(
    o: &O,
    world: World<'_, L>,
    pyramid: &mut Pyramid,
    w: &mut W,
    c: [u32; 2],
) {
    if !o.needs(&*w, Level::Corner, c) {
        return;
    }
    let aim = o.corner(c);
    let k = idx(o.corners()[0], c);
    let base = o.line(k);
    o.keep_origin(w, base, aim.origin);
    let tag = pyramid::tag(pyramid_at(pyramid, pyramid::FRAME), o.passes()[0]);
    let met = cast::walk_from_root(
        &mut Writing {
            world,
            pyramid: &mut *pyramid,
            tag,
            assets: o.assets(),
        },
        aim.origin,
        &aim.ray,
        aim.detail,
        Until::Beam,
        &mut Leaving(&mut *w, base),
    );
    add_tally(pyramid, o.tally(), tally::BEAM, met.visits);
    w.set(o.hand_at(k), o.handed(&aim, met.hit.t).to_bits());
    w.set(base + beam::LINE_VISITS, met.visits);
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
    if !o.needs(&*w, Level::Cell, c) {
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

/// The walk of the ray aimed `aim` to the cell that fills, resumed at
/// cell `cell`'s start from the deepest node of its line holding where it
/// is there, held from the ray's own origin, or from the root at it where
/// that lies in another root (beam.rs `resume_walk`), where its window's
/// beam rays asked as its rays ask, else walked from the world's root
/// (`cell`, `beam::CELL_RESUMES`), as a ray whose sample asked otherwise
/// walks its own (`took`); through `q`, reading or writing, its line left
/// where it ends to `keep`. Its met, and the nodes the line's beam ray
/// visited, none from the root.
pub(crate) fn resume<'a, O: Observer, Q: Query, W: Words + ?Sized>(
    o: &O,
    q: &mut Q,
    w: &'a W,
    aim: &Aim,
    cell: [u32; 2],
    keep: &mut impl cast::Keep<Q::At>,
) -> (Trace<Q::At>, u32)
where
    BeamLine<'a, W>: cast::Over<Q::At>,
{
    let at = o.cell_at(cell);
    let line = BeamLine(w, o.line(w.word(at + beam::CELL_CORNER)));
    let resumes = w.word(at + beam::CELL_RESUMES) != 0;
    let t = f32::from_bits(w.word(at + beam::CELL_T));
    // The line's node, read only where the walk resumes from it.
    let node = if !resumes {
        cast::Node::ROOT
    } else if o.apart() {
        line.node().moved(o.origin_of(w, line.1), aim.origin)
    } else {
        line.node()
    };
    let met = resume_walk(q, aim.origin, &aim.ray, aim.detail, t, &line, node, resumes, keep);
    (met, if resumes { line.visits() } else { 0 })
}

/// Sample `s`'s hit written as a box for the raster's draw (raster.rs
/// `write_box`), the one across and up before it, each where the cast needs
/// it, so it was sampled this cast: its place among the boxes, `NONE` where
/// it wrote none.
pub(crate) fn sample_box<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    s: [u32; 2],
    boxes: &mut [u32],
    args: &mut [u32],
) -> u32 {
    let before = [
        if s[0] > 0 && o.needs(w, Level::Sample, [s[0] - 1, s[1]]) {
            o.sample_at([s[0] - 1, s[1]])
        } else {
            NONE
        },
        if s[1] > 0 && o.needs(w, Level::Sample, [s[0], s[1] - 1]) {
            o.sample_at([s[0], s[1] - 1])
        } else {
            NONE
        },
    ];
    let (origin, eyed) = o.raster_layout();
    write_box(
        w,
        o.sample_at(s),
        before,
        origin,
        idx(o.samples()[0], s),
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
/// hit and that sample's first word; else the sky where no sample about it met
/// anything and none was visible (`sky`); else a walk of its own, which its
/// pass lists for the walk again. The eye's pixels and the light's slots alike.
pub(crate) fn resolve<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    at: O::At,
    aim: &Aim,
    visible: u32,
) -> (Take, cast::Hit<u32>, u32) {
    if visible != 0 && visible & raster::LACK == 0 {
        let (hit, k, enters) = took(o, w, aim, visible);
        if enters {
            return (Take::Sample, hit, k);
        }
    }
    let none = cast::Hit {
        t: aim.ray.t_max,
        axis: 0,
        node: cast::Node::ROOT,
        at: 0u32,
        asset: [0; 2],
    };
    if visible == 0 && sky(o, w, at, aim) {
        (Take::Sky, none, 0)
    } else {
        (Take::Walk, none, 0)
    }
}

/// The cell the ray aimed `aim` takes off the visible sample `visible`, a
/// sample's number that lacks not (`raster::sample_of`): where it enters it
/// as that sample entered it (raster.rs `visible_cell`), the sample's first word,
/// and whether it does, which it does only where the sample asked as the
/// ray asks (`asks_alike`).
pub(crate) fn took<O: Observer, W: Words + ?Sized>(
    o: &O,
    w: &W,
    aim: &Aim,
    visible: u32,
) -> (cast::Hit<u32>, u32, bool) {
    let (n, s) = (o.samples()[0], raster::sample_of(visible));
    let at = [s % n, s / n];
    let k = o.sample_at(at);
    let (hit, enters) = visible_cell(w, k, aim.origin, &aim.ray);
    (hit, k, enters && asks_alike(o, at, aim))
}

/// Whether the four samples about `at` each went on to its reach
/// (`beam::SAMPLE_NONE`), meeting nothing, each asked as the ray aimed
/// `aim` there asks (`asks_alike`): the ray there takes the sky without a
/// walk of its own, as a thing thinner than the samples' spacing between
/// them goes unseen. None where the four lie not about it.
pub(crate) fn sky<O: Observer, W: Words + ?Sized>(o: &O, w: &W, at: O::At, aim: &Aim) -> bool {
    let ([i, j], about) = o.sample_before(at);
    about
        && went_on(o, w, [i, j], aim)
        && went_on(o, w, [i + 1, j], aim)
        && went_on(o, w, [i, j + 1], aim)
        && went_on(o, w, [i + 1, j + 1], aim)
}

/// Whether sample `s` went on to its reach, asked as the ray aimed `aim`
/// asks (`sky`). [rust-gpu] A call of its own, not a closure over `w`, whose
/// capture spirv-val refuses as no logical pointer.
fn went_on<O: Observer, W: Words + ?Sized>(o: &O, w: &W, s: [u32; 2], aim: &Aim) -> bool {
    w.word(o.sample_at(s) + beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE
        && asks_alike(o, s, aim)
}

/// Whether sample `s` was walked at the zooms the ray aimed `aim` asks
/// (`Observer::detail`, `cast::Detail::alike`): what it met is the ray's to
/// take only then.
fn asks_alike<O: Observer>(o: &O, s: [u32; 2], aim: &Aim) -> bool {
    o.detail(o.point(s)).alike(aim.detail)
}

/// The ray at `at` walked again from its cell's start through the observed
/// pyramid writing what it asks, a ray its visible sample and the pyramid left
/// unmet, its line left where it ends to `keep`: its aim, its met and the
/// nodes its beam ray visited.
pub(crate) fn walk_again<O: Observer, W: Words + ?Sized, L: PaletteWords>(
    o: &O,
    world: World<'_, L>,
    pyramid: &mut Pyramid,
    w: &W,
    at: O::At,
    keep: &mut impl cast::Keep<Inheritance>,
) -> (Aim, Trace<Inheritance>, u32) {
    let aim = o.aim(at);
    let tag = pyramid::tag(pyramid_at(pyramid, pyramid::FRAME), o.passes()[2]);
    let (met, before) = resume(
        o,
        &mut Writing {
            world,
            pyramid: &mut *pyramid,
            tag,
            assets: o.assets(),
        },
        w,
        &aim,
        o.cell_of(at),
        keep,
    );
    add_tally(pyramid, o.tally(), tally::MISS, met.visits);
    (aim, met, before)
}
