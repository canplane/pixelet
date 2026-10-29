// pxl-shader/sun.rs

//! The world's depth from the sun into its clipmap (pxl-spec-core `sun`),
//! every slot each frame it is cast, by the passes the eye's rays take,
//! the shadow map their observer (`Map`; observer.rs): the beam rays and
//! their cells' starts, the samples and their boxes, which the raster
//! draws, each the entry the eye's takes too (beam.rs `beam_cs`,
//! `cell_cs`, `sample_cs`; raster.rs `sample_box_cs`) handed the sun's head
//! (`Moved`); every slot taking its visible sample or the sky (`sun_cs`) and
//! the rest walked (`sun_miss_cs`).

use pxl_spec_core::cast::{self, Detail, Until};
use pxl_spec_core::fill::put;
use pxl_spec_core::sun::{Starts, Sun};
use pxl_spec_core::{at, beam, observed, tally, way};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::{spirv, Image};

use crate::beam::Words;
use crate::cast::{add_cnt, enlist, px_tan, Rays};
use crate::decor::Instances;
use crate::eye::{eye_origin, eye_ray, Eye};
use crate::observed::Seen;
use crate::observer::{self, Aim, Level, Observer, Take};
use crate::world::World;

/// The world's depth from the sun, a slot of the sun's clipmap each
/// (pxl-spec-core `sun`): the slot's texel of the light box, and its ray
/// from the box's near plane along the sun's rays, its cell taken off the
/// visible sample or walked through the world's one query to the cell that
/// fills (`cast::first`) as the eye's are, and the word it leaves there,
/// every frame the map is cast. Its one layer read and written in place,
/// the one format family WebGPU lets a shader do both to, as the rays' are.
type SunWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The visible sample at each texel of the box, in the
/// box's order (pxl-spec-core `sun::Sun::texel`): its sample's number
/// (`raster::id_of`), 0 where it kept none, read as an array of one layer,
/// as the eye's pixels read theirs.
type Visibility = Image!(2D, type = u32, sampled = true, arrayed = true);

/// What the sun's cast is handed beside the eye's words, a uniform
/// (pxl-spec-core `sun::MOVED_HEAD`): its number, whether it is counted,
/// its epoch and the map's side, where the rays start (`sun::Starts`) and
/// the actors drawn. The eye's head, all zero, names no map (`size`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Moved {
    v: [IVec4; pxl_spec_core::sun::MOVED_HEAD],
}

const _: () = assert!(
    pxl_spec_core::sun::MOVED_CAST == 0
        && pxl_spec_core::sun::MOVED_COUNTED == 1
        && pxl_spec_core::sun::MOVED_EPOCH == 2
        && pxl_spec_core::sun::MOVED_SIZE == 3,
    "the head's first vec4 is read by its words"
);

/// Where the rays start (`sun::Starts`), off the head.
fn starts(m: &Moved) -> Starts {
    Starts::of_words(|k| {
        let v = m[pxl_spec_core::sun::STARTS_AT + k];
        [v.x, v.y, v.z, v.w]
    })
}

fn counts(m: &Moved) -> bool {
    m[0].y != 0
}

/// The cast's number (`sun::MOVED_CAST`).
fn cast_number(m: &Moved) -> u32 {
    m[0].x as u32
}

/// The number of the cast since which only the eye's look moved
/// (`sun::MOVED_EPOCH`).
fn epoch(m: &Moved) -> u32 {
    m.v[0].z as u32
}

/// The map's side (`sun::MOVED_SIZE`), none in the eye's head.
pub(crate) fn size(m: &Moved) -> u32 {
    m.v[0].w as u32
}

/// Whether a tile from `lo` to `hi`, tiles across and down, is one the
/// cast numbered `cast` casts (`sun::reads_at`), none past the box: tile by
/// tile where they are few, else by the squares they lie in
/// (`sun::coarse_at`), which may name a square for a tile of it alone.
fn casts_in<W: Words + ?Sized>(w: &W, size: u32, cast: u32, lo: [i32; 2], hi: [i32; 2]) -> bool {
    let n = pxl_spec_core::sun::tiles(size) as i32;
    let (lo, hi) = (
        [lo[0].max(0), lo[1].max(0)],
        [hi[0].min(n - 1), hi[1].min(n - 1)],
    );
    if lo[0] > hi[0] || lo[1] > hi[1] {
        return false;
    }
    let few = (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) <= 16;
    let (k, first, row) = if few {
        (1, pxl_spec_core::sun::reads_at(size), n)
    } else {
        (
            pxl_spec_core::sun::COARSE as i32,
            pxl_spec_core::sun::coarse_at(size),
            pxl_spec_core::sun::coarse(size) as i32,
        )
    };
    let mut j = lo[1] / k;
    while j <= hi[1] / k {
        let mut i = lo[0] / k;
        while i <= hi[0] / k {
            if w.word(first + (j * row + i) as u32) == cast {
                return true;
            }
            i += 1;
        }
        j += 1;
    }
    false
}
const _: () = assert!(core::mem::size_of::<SunMoved>() == at_word(pxl_spec_core::sun::MOVED_WORDS));

/// The world as the sun's walks read it, no looks drawn.
fn world<'a>(
    eye: &'a Eye,
    window: &'a [IVec4],
    ground: &'a [u32],
    shards: &'a [IVec4],
    overwrites: &'a [u32],
    pool: &'a [u32],
) -> World<'a, Unlooked> {
    World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    }
}

/// The sun's cast off the eye's words (pxl-spec-core `sun::Sun::words`).
fn sun_of(eye: &Eye) -> pxl_spec_core::sun::Sun {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    pxl_spec_core::sun::Sun {
        first: v3(eye.sun[0]),
        reach: eye.sun[0].w,
        across: v3(eye.sun[1]),
        down: v3(eye.sun[2]),
        dir: v3(eye.sun[3]),
        bias: v3(eye.sun[4]),
        corner: [i(eye.sun[1]), i(eye.sun[2])],
        kept: [i(eye.sun[3]), i(eye.sun[4])],
    }
}

/// The shadow map as an observer (observer.rs `Observer`), off the eye's
/// words and the head the cast is handed: the sun's cast (`sun::Sun`),
/// where its rays start (`sun::Starts`), its side, and the eye's place and
/// zooms its rays ask at, with its beam rays' spacing (`Sun::detail`). Its
/// lattice is the box's texels, a slot each: a beam ray every
/// `sun::BEAM`-th, `sun::BEAM_WINDOW` past the box on each side, a sample
/// every `param::sun::SAMPLE`-th slot, which wrap as the slots do.
pub(crate) struct Map {
    sun: Sun,
    starts: Starts,
    size: u32,
    eye: [i32; 3],
    detail: Detail,
    /// The cast's number (`sun::MOVED_CAST`), which names the tiles it
    /// casts (`Observer::needs`).
    cast: u32,
}

impl Map {
    fn of(eye: &Eye, moved: &Moved, size: u32) -> Map {
        let sun = sun_of(eye);
        let (origin, detail) = eye_origin(eye);
        Map {
            sun,
            starts: starts(moved),
            size: size(moved),
            eye: origin,
            detail: sun.detail(detail),
            cast: cast_number(moved),
        }
    }

    /// Its side (`sun::MOVED_SIZE`), which its slots and its list are laid
    /// by.
    pub(crate) fn side(&self) -> u32 {
        self.size
    }

    /// Its samples across and down, a side.
    fn across(&self) -> u32 {
        self.size / pxl_spec_core::param::sun::SAMPLE
    }

    /// The word a slot leaves for the walk `met` of its ray aimed `aim`.
    fn word<A>(&self, aim: &Aim, met: &cast::Met<A>) -> u32 {
        self.sun
            .word(aim.detail, pxl_spec_core::sun::depth(&self.sun, met))
    }
}

impl Observer for Map {
    type At = [u32; 2];
    const TALLY: u32 = tally::SUN;
    const ASSETS: bool = false;
    const PASSES: [u32; 3] = [
        observed::SUN_BEAM_PASS,
        observed::SUN_SAMPLES_PASS,
        observed::SUN_MISSED_PASS,
    ];
    const APART: bool = true;

    fn corners(&self) -> [u32; 2] {
        let a = pxl_spec_core::sun::beam_across(self.size);
        [a, a]
    }

    /// From where its texel of the box, which may lie past it, lies on the
    /// near plane (`sun::Starts`).
    fn corner(&self, c: [u32; 2]) -> Aim {
        let (b, w) = (
            pxl_spec_core::sun::BEAM as i32,
            pxl_spec_core::sun::BEAM_WINDOW as i32,
        );
        let p = [(c[0] as i32 - w) * b, (c[1] as i32 - w) * b];
        let o = self.starts.start([
            self.sun.corner[0].wrapping_add(p[0]),
            self.sun.corner[1].wrapping_add(p[1]),
        ]);
        let q3 = (1u32 << pxl_spec_core::FRAC_W) as f32;
        let mut detail = self.detail;
        detail.eye = [
            self.eye[0].wrapping_sub(o[0]) as f32 / q3,
            self.eye[1].wrapping_sub(o[1]) as f32 / q3,
            self.eye[2].wrapping_sub(o[2]) as f32 / q3,
        ];
        Aim {
            origin: o,
            ray: cast::Ray::new(self.sun.dir, self.sun.reach),
            detail,
        }
    }

    /// Its stop less `sun::margin`, none before the near plane.
    fn handed(&self, aim: &Aim, t: f32) -> f32 {
        let width = aim.detail.spacing.width(0.0);
        let t = t - pxl_spec_core::sun::margin(width);
        if t > 0.0 {
            t
        } else {
            0.0
        }
    }

    fn line(&self, idx: u32) -> u32 {
        pxl_spec_core::sun::line(self.size, idx)
    }

    /// Every ray of the sun's starts apart, its line's start kept after it
    /// (`sun::LINE_ORIGIN`).
    fn origin_of<W: Words + ?Sized>(&self, w: &W, base: u32) -> [i32; 3] {
        let k = base + pxl_spec_core::sun::LINE_ORIGIN;
        [w.word(k) as i32, w.word(k + 1) as i32, w.word(k + 2) as i32]
    }

    fn keep_origin<W: Words + ?Sized>(&self, w: &mut W, base: u32, o: [i32; 3]) {
        let k = base + pxl_spec_core::sun::LINE_ORIGIN;
        w.set(k, o[0] as u32);
        w.set(k + 1, o[1] as u32);
        w.set(k + 2, o[2] as u32);
    }

    fn cells(&self) -> [u32; 2] {
        let a = pxl_spec_core::sun::cells_across(self.size);
        [a, a]
    }

    /// `sun::BEAM_WINDOW` beam rays about its corners, its own the window's
    /// `BEAM_WINDOW`-th.
    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32) {
        (c, 2 + 2 * pxl_spec_core::sun::BEAM_WINDOW)
    }

    fn cell_at(&self, c: [u32; 2]) -> u32 {
        pxl_spec_core::sun::cell(self.size, c)
    }

    fn samples(&self) -> [u32; 2] {
        [self.across(), self.across()]
    }

    fn point(&self, s: [u32; 2]) -> [u32; 2] {
        let d = pxl_spec_core::param::sun::SAMPLE;
        [s[0] * d, s[1] * d]
    }

    fn sample_cell(&self, s: [u32; 2]) -> [u32; 2] {
        self.cell_of(self.point(s))
    }

    /// Its number on the slots' lattice, wrapped as the slots are: in range
    /// where the samples about a slot lie about it (`sample_before`).
    fn sample_at(&self, s: [u32; 2]) -> u32 {
        let n = self.across();
        pxl_spec_core::sun::sample(self.size, (s[1] % n) * n + s[0] % n)
    }

    /// Not about it within a sample of the box's edge, where the slots wrap:
    /// a sample past it lies at the box's other side.
    fn sample_before(&self, slot: [u32; 2]) -> ([u32; 2], bool) {
        let d = pxl_spec_core::param::sun::SAMPLE;
        let p = self.sun.texel(self.size, slot);
        let (r, n) = ([slot[0] % d, slot[1] % d], self.size);
        let inside = p[0] >= r[0] && p[0] - r[0] + d < n && p[1] >= r[1] && p[1] - r[1] + d < n;
        ([slot[0] / d, slot[1] / d], inside)
    }

    /// From where its texel of the box lies on the near plane (`sun::ray`).
    fn aim(&self, slot: [u32; 2]) -> Aim {
        let p = self.sun.texel(self.size, slot);
        let (origin, ray, from) = pxl_spec_core::sun::ray(self.eye, &self.sun, &self.starts, p);
        let mut detail = self.detail;
        detail.eye = from;
        Aim {
            origin,
            ray,
            detail,
        }
    }

    /// The cell of the box its texel lies in.
    fn cell_of(&self, slot: [u32; 2]) -> [u32; 2] {
        let p = self.sun.texel(self.size, slot);
        let b = pxl_spec_core::sun::BEAM;
        [p[0] / b, p[1] / b]
    }

    /// From where box texel (0, 0)'s ray starts, as the raster lays the
    /// texels' rays off it (pxl-render raster.wgsl), in the frame the slots'
    /// rays start in; none about its eye, as every ray starts on the near
    /// plane.
    fn laid(&self) -> ([i32; 3], (bool, [f32; 2])) {
        (self.starts.start(self.sun.corner), (false, [0.0; 2]))
    }

    /// What feeds a tile this cast casts (`sun::reads_at`): a ray its own
    /// slot's tile; a sample the slots that may take its box, a tile
    /// either way, beyond the four it lies about (`sample_before`), as a box
    /// fewer is a slot walked more; a cell, a tile, as a sample does; a beam
    /// ray the cells whose window holds it (`window`), each as a cell does.
    fn needs<W: Words + ?Sized>(&self, w: &W, level: Level, at: [u32; 2]) -> bool {
        const _: () = assert!(
            pxl_spec_core::sun::BEAM == pxl_spec_core::sun::TILE
                && pxl_spec_core::param::sun::SAMPLE <= pxl_spec_core::sun::TILE,
            "a cell is a tile, and a sample's slots lie within a tile of it"
        );
        let t = pxl_spec_core::sun::TILE as i32;
        let (lo, hi) = match level {
            Level::Ray => {
                let p = self.sun.texel(self.size, at);
                let tile = [p[0] as i32 / t, p[1] as i32 / t];
                (tile, tile)
            }
            Level::Cell => {
                let c = [at[0] as i32, at[1] as i32];
                ([c[0] - 1, c[1] - 1], [c[0] + 1, c[1] + 1])
            }
            Level::Corner => {
                let side = self.window([0, 0]).1 as i32;
                let c = [at[0] as i32, at[1] as i32];
                ([c[0] - side, c[1] - side], [c[0] + 1, c[1] + 1])
            }
            Level::Sample => {
                let p = self.sun.texel(self.size, self.point(at));
                let tile = [p[0] as i32 / t, p[1] as i32 / t];
                ([tile[0] - 1, tile[1] - 1], [tile[0] + 1, tile[1] + 1])
            }
        };
        casts_in(w, self.size, self.cast, lo, hi)
    }
}

const _: () = assert!(
    beam::CELL_T == pxl_spec_core::sun::CELL_T && beam::CELL_CORNER == pxl_spec_core::sun::CELL_RAY,
    "a cell's start lies as the eye's (observer.rs `cell`)"
);

const _: () = assert!(
    pxl_spec_core::sun::RIM == 15,
    "`sun_of` reads the rim off the rays' direction's fourth word"
);

const _: () = assert!(
    pxl_spec_core::sun::TILE == 8,
    "`sun_cs` is a workgroup a tile, its threads a side of it"
);

/// The world's depth from the sun into every slot: a workgroup a tile
/// (`sun::TILE`), a thread a slot, each taking the visible sample where its
/// own ray enters it (observer.rs `took`), or the sky where no sample about
/// it met anything (observer.rs `sky`), as the eye's pixels do (cast.rs
/// `cast_cs`), and the word it leaves there; the rest listed for their walk
/// (`sun_miss_cs`), every slot where the raster is not made yet (observer.rs
/// `resolve`). Only the slots of the tiles the cast casts (`Map::needs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cs"))]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(descriptor_set = 0, binding = 15)] visibility: &Visibility,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] misses: &mut [u32],
) {
    let t = pxl_spec_core::sun::TILE;
    let size: UVec3 = sun.query_size();
    let slot = UVec3::new(group.x * t + lane.x, group.y * t + lane.y, 0);
    if slot.x >= size.x || slot.y >= size.y {
        return;
    }
    let m = Map::of(eye, moved, size.x);
    let at = [slot.x, slot.y];
    // A slot no receiver reads, or one an earlier cast of the epoch cast,
    // left as it was.
    if !m.needs(&*list, Level::Ray, at) {
        return;
    }
    let aim = m.aim(at);
    let p = m.sun.texel(size.x, at);
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = visibility.fetch(UVec3::new(p[0], p[1], 0));
    let (took, stop, _) = observer::resolve(&m, &*list, at, &aim, kept.x);
    if took == Take::Sample {
        let met = cast::Met {
            ends: cast::End::Stop,
            stop,
            visits: 0,
        };
        add_cnt(misses, counts(moved), tally::TOOK, 1);
        // SAFETY: within the map, bounded by its size above.
        unsafe { sun.write(slot, m.word(&aim, &met)) };
        return;
    }
    if took == Take::Sky {
        // SAFETY: within the map, bounded by its size above.
        unsafe { sun.write(slot, m.sun.word(aim.detail, 1.0)) };
        return;
    }
    // Listed for its walk again as the eye's pixels are (cast.rs `enlist`):
    // the list holds every slot, so none finds it full.
    add_cnt(misses, counts(moved), tally::WALKED, 1);
    let at = enlist(misses, n * n, [0; 2]);
    put(
        list,
        pxl_spec_core::sun::misses_at(size.x) + at,
        slot.y << 16 | slot.x,
    );
}

const _: () = assert!(
    pxl_spec_core::ray::MISS_LANES == 64,
    "`sun_miss_cs` is a workgroup of `ray::MISS_LANES` slots, as `miss_list_cs` is"
);

/// The world's depth from the sun into each slot `sun_cs` listed: a thread
/// a slot, `ray::MISS_LANES` a workgroup and `ray::MISS_ACROSS` workgroups
/// a row, each past the count returning at once, as the eye's listed pixels
/// are (cast.rs `miss_list_cs`), its ray walked again (observer.rs
/// `walked`), and the word it leaves there.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "sun_miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &SunMoved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
) {
    let window: &[IVec4] = window;
    let m = Map::of(eye, moved);
    let n = m.side();
    let i = (group.y * pxl_spec_core::ray::MISS_ACROSS + group.x) * pxl_spec_core::ray::MISS_LANES
        + lane.x;
    if i >= at(list, pxl_spec_core::sun::misses_cnt(n)) {
        return;
    }
    let tile = at(list, pxl_spec_core::sun::LIST_AT as u32 + k);
    let slot = UVec3::new((tile & 0xffff) * t + lane.x, (tile >> 16) * t + lane.y, 0);
    if slot.x >= size.x || slot.y >= size.y {
        return;
    }
    let (walks, o, ray, detail) = sun_slot(eye, sun, moved, size.x, slot);
    if !walks {
        return;
    }
    let w = at(list, pxl_spec_core::sun::misses_at(size.x) + i);
    let slot = UVec3::new(w & 0xffff, w >> 16, 0);
    let world = world(eye, window, ground, shards, overwrites, pool);
    let m = Map::of(eye, moved, size.x);
    let (aim, met, _) = observer::walked(&m, world, seen, list, [slot.x, slot.y]);
    // SAFETY: a slot `sun_cs` listed, within the map.
    unsafe { sun.write(slot, m.word(&aim, &met)) };
}

/// The world's receivers of the shadow map: the drawn pixels' rays (cast.rs
/// `Rays`), a thread a square of them `sun::MARK` a side, each read, the
/// tile the point it met the world at lies on marked stood on
/// (`sun::stands_at`), a run on one tile once; nothing where it met
/// nothing. A word written alike by every thread it is marked by, so none
/// needs an atomic. Run after the eye's rays, before the reach
/// (`sun_reach_cs`); the drawn receivers stand apart (`sun_mark_drawn_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_mark_cs"))]
pub fn sun_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = sun.query_size();
    let m = Map::of(eye, moved, size.x);
    if id.x >= m.across() || id.y >= m.across() {
        return;
    }
    let (s, n) = (sun_of(eye), size(moved));
    let cast = cast_number(moved);
    // The tile last marked, so a run of pixels on one marks it once.
    let mut last = [i32::MIN; 2];
    let mut y = 0;
    while y < m {
        let mut x = 0;
        while x < m {
            let p = UVec3::new(id.x * m + x, id.y * m + y, 0);
            // A ray that met nothing leaves its first word zero (cast.rs
            // `word`).
            let first: u32 = if p.x < drawn.x && p.y < drawn.y {
                rays.read(UVec3::new(p.x, p.y, 0))
            } else {
                0
            };
            if first != 0 && first != pxl_spec_core::ray::MISS {
                let t = f32::from_bits(rays.read(UVec3::new(p.x, p.y, 1)));
                let d = eye_ray(eye, px_tan(eye, p, drawn)).dir;
                let tile = s.tile_of([d[0] * t, d[1] * t, d[2] * t]);
                // [rust-gpu] By its words: an array's `!=` is an intrinsic
                // it does not lower.
                if tile[0] != last[0] || tile[1] != last[1] {
                    stand(list, n, cast, tile);
                    last = tile;
                }
            }
            x += 1;
        }
        y += 1;
    }
    let n = size.x / d;
    let at = |i: u32, j: u32| pxl_spec_core::sun::sample(size.x, j * n + i);
    let before = [
        if id.x > 0 { at(id.x - 1, id.y) } else { NONE },
        if id.y > 0 { at(id.x, id.y - 1) } else { NONE },
    ];
    // From where box texel (0, 0)'s ray starts, as the raster lays the
    // texels' rays off it (pxl-render raster.wgsl), in the frame the slots'
    // rays start in (`sun::Starts`).
    let s = sun_of(eye);
    let origin = starts(moved).start(s.corner);
    let box_at = boxed(
        list,
        at(id.x, id.y),
        before,
        origin,
        id.y * n + id.x,
        (false, [0.0; 2]),
        boxes,
        args,
    );
    if box_at != NONE && counted(moved) {
        add(counts, pxl_spec_core::sun::COUNT_BOXES, 1);
    }
}

/// Each beam ray of the sun's map (pxl-spec-core `sun::BEAM`), a thread
/// each, `sun::beam_across` a side, `sun::BEAM_WINDOW` of them past the box
/// on each side, the map's (observer.rs `beam`): its line and its origin
/// left for the texels about it, and how far they start from it.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = sun.query_size();
    let m = Map::of(eye, moved, size.x);
    let [across, down] = m.corners();
    if id.x >= across || id.y >= down {
        return;
    }
    let world = world(eye, window, ground, shards, overwrites, pool);
    observer::beam(&m, world, seen, list, [id.x, id.y]);
}

/// Where each cell of the sun's map starts (pxl-spec-core `sun::cell`), a
/// thread a cell, `sun::BEAM` texels a side, the map's (observer.rs
/// `cell`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cell_cs"))]
pub fn sun_cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let size: UVec3 = sun.query_size();
    let m = Map::of(eye, moved, size.x);
    let [across, down] = m.cells();
    // [naga] The eye's vec2 of u32s read, so the module holds the type
    // naga's SPIR-V reader splits an arrayed image's layer off by (`Eye`).
    if id.x >= across || id.y >= down || eye.range.x > eye.range.y {
        return;
    }
    let across = pxl_spec_core::sun::beam_across(size.x);
    let side = 2 + 2 * pxl_spec_core::sun::BEAM_WINDOW;
    // Its own corners lie `BEAM_WINDOW` in from the rays' first.
    let first = id.y * across + id.x;
    let mut from = first;
    let mut least = f32::from_bits(at(
        list,
        pxl_spec_core::sun::line(size.x, first) + beam::LINE_T,
    ));
    let mut j = 0;
    while j < side {
        let mut i = 0;
        while i < side {
            let r = (id.y + j) * across + id.x + i;
            let t = f32::from_bits(at(list, pxl_spec_core::sun::line(size.x, r) + beam::LINE_T));
            if t < least {
                least = t;
                from = r;
            }
            i += 1;
        }
        j += 1;
    }
    let c = pxl_spec_core::sun::cell(size.x, [id.x, id.y]);
    put(list, c + pxl_spec_core::sun::CELL_T, least.to_bits());
    put(list, c + pxl_spec_core::sun::CELL_RAY, from);
}

/// Tile `[i, j]`'s word among the box's tiles' words at `first` (the
/// stands, the reads), `NO_TILE` past the box.
fn tile_word(size: u32, first: u32, [i, j]: [i32; 2]) -> u32 {
    let n = pxl_spec_core::sun::tiles(size) as i32;
    if i >= 0 && j >= 0 && i < n && j < n {
        first + (j * n + i) as u32
    } else {
        NO_TILE
    }
}

/// No tile's word (`tile_word`).
const NO_TILE: u32 = u32::MAX;

/// Tile `tile` of the box marked stood on in the cast numbered `cast`
/// (`sun::stands_at`), none past the box.
fn stand(list: &mut [u32], size: u32, cast: u32, tile: [i32; 2]) {
    let w = tile_word(size, pxl_spec_core::sun::stands_at(size), tile);
    if w != NO_TILE {
        put(list, w, cast);
    }
}

/// The drawn receivers of the shadow map, each the eye sees standing as the
/// world's do (`sun_mark_cs`): the actors the host draws, a thread each
/// (`sun::ACTORS_AT`); then the decor grown, a thread a slot of its
/// instances (pxl-render gpu/grown.rs), as many as are placed, where each
/// stands, as decor tells (decor.rs `Instances`). Seen where a
/// ball about its middle as wide as decor is tall and wide meets the view
/// (`eye::sees`); the tiles about its own stood on too, as many as half
/// the widest decor spans (`Sun::ring`). The actors are those the scene
/// draws in the world (pxl-render gpu/renderer.rs `Cast::actors`), no more
/// than `sun::ACTORS`. Run after decor is placed, before the reach.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "sun_mark_drawn_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_mark_drawn_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] regions: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] instances: &[u32],
) {
    use pxl_spec_core::scatter::{TALLEST, WIDEST};
    let head = pxl_spec_core::sun::ACTORS_AT;
    let k = id.x as usize;
    let v = if k < pxl_spec_core::sun::ACTORS {
        if k >= moved[head].x as usize {
            return;
        }
        let p = moved[head + 1 + k];
        [
            f32::from_bits(p.x as u32),
            f32::from_bits(p.y as u32),
            f32::from_bits(p.z as u32),
        ]
    } else {
        let placed = Instances {
            drawing,
            regions,
            instances,
        };
        let k = (k - pxl_spec_core::sun::ACTORS) as u32;
        if k >= placed.cnt() {
            return;
        }
        let o = [eye.origin.x, eye.origin.y, eye.origin.z];
        placed.stands(k, o)
    };
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let middle = [v[0], v[1], v[2] + TALLEST as f32 / 2.0];
    let axes = [v3(eye.forward_far), v3(eye.right), v3(eye.up)];
    if !pxl_spec_core::eye::sees(
        axes,
        [eye.tan.x, eye.tan.y],
        middle,
        (TALLEST + WIDEST) as f32,
    ) {
        return;
    }
    let (s, n) = (sun_of(eye), size(moved));
    let ([i, j], cast, r) = (s.tile_of(v), cast_number(moved), s.ring());
    let mut b = -r;
    while b <= r {
        let mut a = -r;
        while a <= r {
            stand(list, n.x, cast, [i + a, j + b]);
            a += 1;
        }
        b += 1;
    }
}

/// Which tiles of the box a receiver reads in this cast, a thread a tile,
/// after every receiver stood (`sun_mark_cs`, `sun_mark_drawn_cs`), before
/// the beam: one in the disk it reads (`Sun::read`), within a tile and its
/// taps of a step up the way up from a tile one stands on (`Sun::up`,
/// `Sun::up_at`: a point that far up lies that many tiles over or one more,
/// its taps a tile either side), as decor and the actors stand on what the
/// eye meets and are no taller. One read that no cast of the epoch read is
/// the cast's to cast: its word names the cast (`sun::reads_at`), and its
/// square's (`sun::coarse_at`). A tile's word its own thread's alone.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_reach_cs"))]
pub fn sun_reach_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let side = size(moved);
    let n = pxl_spec_core::sun::tiles(side);
    if id.x >= n || id.y >= n {
        return;
    }
    let s = sun_of(eye);
    // A tile past every slot a receiver reads holds none of its slots.
    let t = pxl_spec_core::sun::TILE;
    let middle = [(id.x * t + t / 2) as i32, (id.y * t + t / 2) as i32];
    if !s.read(
        size.x,
        middle,
        pxl_spec_core::param::sun::READ_PAD + t as f32 * core::f32::consts::FRAC_1_SQRT_2,
    ) {
        return;
    }
    let (cast, epoch) = (cast_number(moved), epoch(moved));
    let up = s.up();
    let t = [id.x as i32, id.y as i32];
    let mut read = false;
    let mut k = 0;
    while k <= up.1 && !read {
        let o = Sun::up_at(up, k);
        let mut b = -2;
        while b <= 1 && !read {
            let mut a = -2;
            while a <= 1 && !read {
                read = stood(&*list, size.x, cast, [t[0] - o[0] + a, t[1] - o[1] + b]);
                a += 1;
            }
            b += 1;
        }
        k += 1;
    }
    // Read, and read first in the epoch: the cast's to cast.
    let w = pxl_spec_core::sun::reads_at(size.x) + id.y * n + id.x;
    if read && at(list, w) < epoch {
        put(list, w, cast);
        put(
            list,
            pxl_spec_core::sun::coarse_square(size.x, [id.x, id.y]),
            cast,
        );
    }
}

/// Whether a receiver stood on tile `tile` in the cast numbered `cast`,
/// none past the box.
fn stood(list: &[u32], size: u32, cast: u32, tile: [i32; 2]) -> bool {
    let w = tile_word(size, pxl_spec_core::sun::stands_at(size), tile);
    w != NO_TILE && at(list, w) == cast
}
