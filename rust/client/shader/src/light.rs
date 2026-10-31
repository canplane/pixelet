// pxl-shader/light.rs

//! The world's depth from the light into its clipmap (pxl-spec-core `light`),
//! every slot each frame it is cast, by the passes the eye's rays take,
//! the shadow map their observer (`Map`; observer.rs): the beam rays and
//! their cells' starts, the samples and their boxes, which the raster
//! draws, each the entry the eye's takes too (beam.rs `beam_cs`,
//! `cell_cs`, `sample_cs`; raster.rs `sample_box_cs`) handed the light's head
//! (`Head`); every slot taking its visible sample or the sky (`light_cs`) and
//! the rest walked (`light_miss_cs`).

use pxl_spec_core::cast::{self, Detail, Margin};
use pxl_spec_core::fill::put;
use pxl_spec_core::light::{Light, Starts};
use pxl_spec_core::{at, beam, observed, tally, way};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::{spirv, Image};

use crate::beam::{Beams, Words};
use crate::cast::{add_cnt, enlist, px_tan, Rays};
use crate::decor::Instances;
use crate::eye::{eye_origin, eye_ray, Eye};
use crate::observed::Seen;
use crate::observer::{self, Aim, Level, Observer, Take};
use crate::world::World;

/// The world's depth from the light, a slot of the light's clipmap each
/// (pxl-spec-core `light`): the slot's texel of the light box, and its ray
/// from the box's near plane along the light's rays, its cell taken off the
/// visible sample or walked through the world's one query to the cell that
/// fills (`cast::first`) as the eye's are, and the word it leaves there,
/// every frame the map is cast. Its one layer read and written in place,
/// the one format family WebGPU lets a shader do both to, as the rays' are.
type LightWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The visible sample at each texel of the box, in the
/// box's order (pxl-spec-core `light::Light::texel`): its sample's number
/// (`raster::id_of`), 0 where it kept none, read as an array of one layer,
/// as the eye's pixels read theirs.
type Visibility = Image!(2D, type = u32, sampled = true, arrayed = true);

/// What the light's cast is handed beside the eye's words, a uniform
/// (pxl-spec-core `light::HEAD_VEC4S`): its number, whether it is counted,
/// its epoch and the map's side, where the rays start (`light::Starts`) and
/// the actors drawn. The eye's head, all zero, names no map (`size`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Head {
    v: [IVec4; pxl_spec_core::light::HEAD_VEC4S],
}

const _: () = assert!(
    pxl_spec_core::light::HEAD_CAST == 0
        && pxl_spec_core::light::HEAD_COUNTED == 1
        && pxl_spec_core::light::HEAD_EPOCH == 2
        && pxl_spec_core::light::HEAD_SIZE == 3
        && pxl_spec_core::light::HEAD_CORNER_WAS == 4
        && pxl_spec_core::light::HEAD_FOOTPRINT == 6,
    "the head's first two vec4s are read by their words"
);

/// Where the rays start (`light::Starts`), off the head.
fn starts(m: &Head) -> Starts {
    Starts::of_words(|k| {
        let v = m.v[pxl_spec_core::light::STARTS_AT + k];
        [v.x, v.y, v.z, v.w]
    })
}

fn counts(m: &Head) -> bool {
    m.v[0].y != 0
}

/// The cast's number (`light::HEAD_CAST`).
fn cast_number(m: &Head) -> u32 {
    m.v[0].x as u32
}

/// The number of the cast since which only the eye's look moved
/// (`light::HEAD_EPOCH`).
fn epoch(m: &Head) -> u32 {
    m.v[0].z as u32
}

/// Whether the cast's marks take each pixel's footprint
/// (`light::HEAD_FOOTPRINT`).
fn footprint(m: &Head) -> bool {
    m.v[1].z != 0
}

/// The map's side (`light::HEAD_SIZE`), none in the eye's head.
pub(crate) fn size(m: &Head) -> u32 {
    m.v[0].w as u32
}

/// Whether a tile from `lo` to `hi`, tiles across and down, is one the
/// cast numbered `cast` casts (`light::reads_at`), none past the box: tile by
/// tile where they are few, else by the squares they lie in
/// (`light::coarse_at`), which may name a square for a tile of it alone.
fn casts_in<W: Words + ?Sized>(w: &W, size: u32, cast: u32, lo: [i32; 2], hi: [i32; 2]) -> bool {
    let n = pxl_spec_core::light::tiles(size) as i32;
    let (lo, hi) = (
        [lo[0].max(0), lo[1].max(0)],
        [hi[0].min(n - 1), hi[1].min(n - 1)],
    );
    if lo[0] > hi[0] || lo[1] > hi[1] {
        return false;
    }
    let few = (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) <= 16;
    let (k, first, row) = if few {
        (1, pxl_spec_core::light::reads_at(size), n)
    } else {
        (
            pxl_spec_core::light::COARSE as i32,
            pxl_spec_core::light::coarse_at(size),
            pxl_spec_core::light::coarse(size) as i32,
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

/// The world as the light's walk again reads it, as the eye's do, the looks
/// too, which only what a stop is drawn by holds and the map's word leaves
/// out (`Map::word`).
fn world<'a>(
    eye: &'a Eye,
    window: &'a [IVec4],
    ground: &'a [u32],
    looks: &'a [u32],
    shards: &'a [IVec4],
    overwrites: &'a [u32],
    pool: &'a [u32],
) -> World<'a, &'a [u32]> {
    World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    }
}

/// The light's cast off the eye's words (pxl-spec-core `light::Light::words`).
fn light_of(eye: &Eye) -> Light {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    Light {
        first: v3(eye.light[0]),
        t_max: eye.light[0].w,
        across: v3(eye.light[1]),
        down: v3(eye.light[2]),
        dir: v3(eye.light[3]),
        bias: v3(eye.light[4]),
        corner: [i(eye.light[1]), i(eye.light[2])],
        shadow_radius_ndc: eye.light[3].w,
    }
}

/// The shadow map as an observer (observer.rs `Observer`), off the eye's
/// words and the head the cast is handed: the light's cast (`light::Light`),
/// where its rays start (`light::Starts`), its side, and the eye's place and
/// zooms its rays ask at, with its beam rays' spacing (`Light::detail`). Its
/// lattice is the box's texels, a slot each: a beam ray every
/// `light::BEAM`-th, `light::BEAM_WINDOW` past the box on each side, a sample
/// every `param::light::SAMPLE`-th slot, which wrap as the slots do.
pub(crate) struct Map {
    light: Light,
    starts: Starts,
    size: u32,
    eye: [i32; 3],
    detail: Detail,
    /// The cast's number (`light::HEAD_CAST`), which names the tiles it
    /// casts (`Observer::needs`).
    cast: u32,
}

impl Map {
    pub(crate) fn of(eye: &Eye, head: &Head) -> Map {
        let light = light_of(eye);
        let (origin, detail) = eye_origin(eye);
        Map {
            light,
            starts: starts(head),
            size: size(head),
            eye: origin,
            detail: light.detail(detail),
            cast: cast_number(head),
        }
    }

    /// Its side (`light::HEAD_SIZE`), which its slots and its list are laid
    /// by.
    pub(crate) fn side(&self) -> u32 {
        self.size
    }

    /// Its samples across and down, a side.
    fn across(&self) -> u32 {
        self.size / pxl_spec_core::param::light::SAMPLE
    }

    /// The word a slot leaves for the walk `met` of its ray.
    fn word<A>(&self, met: &cast::Met<A>) -> u32 {
        pxl_spec_core::light::word(pxl_spec_core::light::depth(&self.light, met))
    }
}

impl Observer for Map {
    type At = [u32; 2];

    fn tally(&self) -> u32 {
        tally::LIGHT
    }

    /// None: its rays need only a stop's depth, and the pyramid's slots,
    /// a fixed few a bucket (`observed::victim`), are kept for the cells the
    /// eye's rays are drawn by and walk again.
    fn assets(&self) -> bool {
        false
    }

    fn passes(&self) -> [u32; 3] {
        [
            observed::LIGHT_BEAM_PASS,
            observed::LIGHT_SAMPLES_PASS,
            observed::LIGHT_MISSED_PASS,
        ]
    }

    fn apart(&self) -> bool {
        true
    }

    fn corners(&self) -> [u32; 2] {
        let a = pxl_spec_core::light::beam_across(self.size);
        [a, a]
    }

    /// From where its texel of the box, which may lie past it, lies on the
    /// near plane (`light::Starts`).
    fn corner(&self, c: [u32; 2]) -> Aim {
        let (b, w) = (
            pxl_spec_core::light::BEAM as i32,
            pxl_spec_core::light::BEAM_WINDOW as i32,
        );
        let p = [(c[0] as i32 - w) * b, (c[1] as i32 - w) * b];
        let o = self.starts.start([
            self.light.corner[0].wrapping_add(p[0]),
            self.light.corner[1].wrapping_add(p[1]),
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
            ray: cast::Ray::new(self.light.dir, self.light.t_max),
            detail,
        }
    }

    /// Its stop less `light::margin`, none before the near plane.
    fn handed(&self, aim: &Aim, t: f32) -> f32 {
        let width = aim.detail.spacing.width(0.0);
        let t = t - pxl_spec_core::light::margin(width);
        if t > 0.0 {
            t
        } else {
            0.0
        }
    }

    fn line(&self, idx: u32) -> u32 {
        pxl_spec_core::light::line(self.size, idx)
    }

    /// Every ray of the light's starts apart, its line's start kept after it
    /// (`light::LINE_ORIGIN`).
    fn origin_of<W: Words + ?Sized>(&self, w: &W, base: u32) -> [i32; 3] {
        let k = base + pxl_spec_core::light::LINE_ORIGIN;
        [w.word(k) as i32, w.word(k + 1) as i32, w.word(k + 2) as i32]
    }

    fn keep_origin<W: Words + ?Sized>(&self, w: &mut W, base: u32, o: [i32; 3]) {
        let k = base + pxl_spec_core::light::LINE_ORIGIN;
        w.set(k, o[0] as u32);
        w.set(k + 1, o[1] as u32);
        w.set(k + 2, o[2] as u32);
    }

    fn cells(&self) -> [u32; 2] {
        let a = pxl_spec_core::light::cells_across(self.size);
        [a, a]
    }

    /// `light::BEAM_WINDOW` beam rays about its corners, its own the window's
    /// `BEAM_WINDOW`-th.
    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32) {
        (c, 2 + 2 * pxl_spec_core::light::BEAM_WINDOW)
    }

    fn cell_at(&self, c: [u32; 2]) -> u32 {
        pxl_spec_core::light::cell(self.size, c)
    }

    fn samples(&self) -> [u32; 2] {
        [self.across(), self.across()]
    }

    fn point(&self, s: [u32; 2]) -> [u32; 2] {
        let d = pxl_spec_core::param::light::SAMPLE;
        [s[0] * d, s[1] * d]
    }

    fn sample_cell(&self, s: [u32; 2]) -> [u32; 2] {
        self.cell_of(self.point(s))
    }

    /// Its number on the slots' lattice, wrapped as the slots are: in range
    /// where the samples about a slot lie about it (`sample_before`).
    fn sample_at(&self, s: [u32; 2]) -> u32 {
        let n = self.across();
        pxl_spec_core::light::sample(self.size, (s[1] % n) * n + s[0] % n)
    }

    /// Not about it within a sample of the box's edge, where the slots wrap:
    /// a sample past it lies at the box's other side.
    fn sample_before(&self, slot: [u32; 2]) -> ([u32; 2], bool) {
        let d = pxl_spec_core::param::light::SAMPLE;
        let p = self.light.texel(self.size, slot);
        let (r, n) = ([slot[0] % d, slot[1] % d], self.size);
        let inside = p[0] >= r[0] && p[0] - r[0] + d < n && p[1] >= r[1] && p[1] - r[1] + d < n;
        ([slot[0] / d, slot[1] / d], inside)
    }

    /// From where its texel of the box lies on the near plane (`light::ray`).
    fn aim(&self, slot: [u32; 2]) -> Aim {
        let p = self.light.texel(self.size, slot);
        let (origin, ray, from) = pxl_spec_core::light::ray(self.eye, &self.light, &self.starts, p);
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
        let p = self.light.texel(self.size, slot);
        let b = pxl_spec_core::light::BEAM;
        [p[0] / b, p[1] / b]
    }

    /// From where box texel (0, 0)'s ray starts, as the raster lays the
    /// texels' rays off it (pxl-render raster.wgsl), in the frame the slots'
    /// rays start in; none about its eye, as every ray starts on the near
    /// plane.
    fn raster_layout(&self) -> ([i32; 3], (bool, [f32; 2])) {
        (self.starts.start(self.light.corner), (false, [0.0; 2]))
    }

    /// What feeds a tile this cast casts (`light::reads_at`): one of the
    /// tiles it feeds (`feeds`).
    fn needs<W: Words + ?Sized>(&self, w: &W, level: Level, at: [u32; 2]) -> bool {
        let (lo, hi) = self.feeds(level, at);
        casts_in(w, self.size, self.cast, lo, hi)
    }
}

impl Map {
    /// The tiles of the box what `level` at `at` feeds, from the first to
    /// the last across and down: a ray its own slot's tile; a sample the
    /// slots that may take its box, a tile either way, beyond the four it
    /// lies about (`sample_before`), as a box fewer is a slot walked more; a
    /// cell, a tile, as a sample does; a beam ray the cells whose window
    /// holds it (`window`), each as a cell does.
    fn feeds(&self, level: Level, at: [u32; 2]) -> ([i32; 2], [i32; 2]) {
        const _: () = assert!(
            pxl_spec_core::light::BEAM == pxl_spec_core::light::TILE
                && pxl_spec_core::param::light::SAMPLE <= pxl_spec_core::light::TILE,
            "a cell is a tile, and a sample's slots lie within a tile of it"
        );
        let t = pxl_spec_core::light::TILE as i32;
        let (lo, hi) = match level {
            Level::Ray => {
                let p = self.light.texel(self.size, at);
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
                let p = self.light.texel(self.size, self.point(at));
                let tile = [p[0] as i32 / t, p[1] as i32 / t];
                ([tile[0] - 1, tile[1] - 1], [tile[0] + 1, tile[1] + 1])
            }
        };
        (lo, hi)
    }

    /// The least margin `told` of the walk that gave what `level` at `at`
    /// gives (`cast::Margin`), lowered into the held words of each tile it
    /// feeds that this cast casts (`feeds`, `light::held_at`), in the list
    /// `w`: the tile holds while the eye stays nearer than every margin told
    /// it to where it stood (`light_detail_cs`). None where the walk asked
    /// nothing.
    fn told<W: Words + ?Sized>(&self, w: &mut W, level: Level, at: [u32; 2], told: Margin) {
        if told.0 >= Margin::NONE.0 {
            return;
        }
        let n = pxl_spec_core::light::tiles(self.size) as i32;
        let (lo, hi) = self.feeds(level, at);
        let (lo, hi) = (
            [lo[0].max(0), lo[1].max(0)],
            [hi[0].min(n - 1), hi[1].min(n - 1)],
        );
        let reads = pxl_spec_core::light::reads_at(self.size);
        let mut j = lo[1];
        while j <= hi[1] {
            let mut i = lo[0];
            while i <= hi[0] {
                if w.word(reads + (j * n + i) as u32) == self.cast {
                    let h = self.light.held(self.size, [i as u32, j as u32]);
                    w.least(h + pxl_spec_core::light::HELD_MARGIN, told.0.to_bits());
                }
                i += 1;
            }
            j += 1;
        }
    }
}

const _: () = assert!(
    beam::CELL_T == pxl_spec_core::light::CELL_T
        && beam::CELL_CORNER == pxl_spec_core::light::CELL_RAY,
    "a cell's start lies as the eye's (observer.rs `cell`)"
);

const _: () = assert!(
    pxl_spec_core::light::SHADOW_RADIUS_NDC == 15,
    "`light_of` reads the shadow's radius off the rays' direction's fourth word"
);

const _: () = assert!(
    pxl_spec_core::light::TILE == 8,
    "`light_cs` is a workgroup a tile, its threads a side of it"
);

/// The world's depth from the light into every slot: a workgroup a tile
/// (`light::TILE`), a thread a slot, each taking the visible sample where its
/// own ray enters it (observer.rs `took`), or the sky where no sample about
/// it met anything (observer.rs `sky`), as the eye's pixels do (cast.rs
/// `cast_cs`), and the word it leaves there; the rest listed for their walk
/// (`light_miss_cs`), every slot where the raster is not made yet (observer.rs
/// `resolve`). Only the slots of the tiles the cast casts (`Map::needs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_cs"))]
pub fn light_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] light: &LightWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(descriptor_set = 0, binding = 15)] visibility: &Visibility,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] misses: &mut [u32],
) {
    let t = pxl_spec_core::light::TILE;
    let m = Map::of(eye, head);
    let n = m.side();
    let slot = UVec3::new(group.x * t + lane.x, group.y * t + lane.y, 0);
    if slot.x >= n || slot.y >= n {
        return;
    }
    let at = [slot.x, slot.y];
    // A slot no receiver reads, or one an earlier cast of the epoch cast,
    // left as it was.
    if !m.needs(&*list, Level::Ray, at) {
        return;
    }
    let aim = m.aim(at);
    let p = m.light.texel(n, at);
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = visibility.fetch(UVec3::new(p[0], p[1], 0));
    let (took, stop, _) = observer::resolve(&m, &*list, at, &aim, kept.x);
    if took == Take::Sample {
        let met = cast::Met {
            ends: cast::End::Stop,
            stop,
            visits: 0,
        };
        add_cnt(misses, counts(head), tally::TOOK, 1);
        // SAFETY: within the map, bounded by its size above.
        unsafe { light.write(slot, m.word(&met)) };
        return;
    }
    if took == Take::Sky {
        // SAFETY: within the map, bounded by its size above.
        unsafe { light.write(slot, pxl_spec_core::light::word(1.0)) };
        return;
    }
    // Listed for its walk again as the eye's pixels are (cast.rs `enlist`):
    // the list holds every slot, so none finds it full.
    add_cnt(misses, counts(head), tally::WALKED, 1);
    let at = enlist(misses, n * n, [0; 2]);
    put(
        list,
        pxl_spec_core::light::misses_at(n) + at,
        slot.y << 16 | slot.x,
    );
}

/// The world's depth from the light into each slot `light_cs` listed: a thread
/// a slot, `ray::MISS_LANES` a workgroup and `ray::MISS_ACROSS` workgroups
/// a row, each past the count returning at once, as the eye's listed pixels
/// are (cast.rs `miss_list_cs`), its ray walked again (observer.rs
/// `walked`), and the word it leaves there; the least margin of the nodes
/// it asked lowered into its tile's held words (`Map::told`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "light_miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_miss_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] light: &LightWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let m = Map::of(eye, head);
    let n = m.side();
    let i = (group.y * pxl_spec_core::ray::MISS_ACROSS + group.x) * pxl_spec_core::ray::MISS_LANES
        + lane.x;
    if i >= list.word(pxl_spec_core::light::misses_cnt(n)) {
        return;
    }
    let w = list.word(pxl_spec_core::light::misses_at(n) + i);
    let slot = UVec3::new(w & 0xffff, w >> 16, 0);
    let world = world(eye, window, ground, looks, shards, overwrites, pool);
    let mut told = Margin::NONE;
    let at = [slot.x, slot.y];
    let (_, met, _) = observer::walked(&m, world, seen, &*list, at, &mut (), &mut told);
    // SAFETY: a slot `light_cs` listed, within the map.
    unsafe { light.write(slot, m.word(&met)) };
    m.told(list, Level::Ray, at, told);
}

/// Each beam ray of the shadow map's lattice (`light::BEAM`), a thread a
/// corner, as the eye's are (beam.rs `beam_cs`; observer.rs `beam`), its
/// line left in the list; the least margin of the nodes it asked lowered
/// into the held words of the tiles it feeds (`Map::told`). An entry of its
/// own, so the eye's beam rays tell nothing and pay nothing for it.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut Beams,
) {
    let window: &[IVec4] = window;
    let m = Map::of(eye, head);
    let [across, down] = m.corners();
    if id.x >= across || id.y >= down {
        return;
    }
    let world = world(eye, window, ground, looks, shards, overwrites, pool);
    let (c, mut told) = ([id.x, id.y], Margin::NONE);
    observer::beam(&m, world, seen, list, c, &mut told);
    m.told(list, Level::Corner, c, told);
}

/// Each sample of the shadow map's lattice (`param::light::SAMPLE`), a
/// thread each, as the eye's are (beam.rs `sample_cs`; observer.rs
/// `sample`), its stop left in the list for the raster; the least margin
/// of the nodes it asked lowered into the held words of the tiles it feeds
/// (`Map::told`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_sample_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut Beams,
) {
    let window: &[IVec4] = window;
    let m = Map::of(eye, head);
    let [across, down] = m.samples();
    if id.x >= across || id.y >= down {
        return;
    }
    let world = world(eye, window, ground, looks, shards, overwrites, pool);
    let (at, mut told) = ([id.x, id.y], Margin::NONE);
    observer::sample(&m, world, seen, list, at, &mut told);
    m.told(list, Level::Sample, at, told);
}

/// The world's receivers of the shadow map: the drawn pixels' rays (cast.rs
/// `Rays`), a thread a square of them `light::MARK` a side, each read, the
/// tile the point it met the world at lies on marked stood on
/// (`light::stands_at`), a run on one tile once, and the tiles between it
/// and where each pixel beside it met it, across, down and either diagonal,
/// each pair of neighbors once, its footprint (`light::FOOTPRINT_TILES`), so
/// a jittered ray stands on a tile marked whichever way it moves, where the
/// head asks it (`light::HEAD_FOOTPRINT`), else its own tile alone; nothing
/// where it met nothing. A word written alike by every thread it is marked
/// by, so none needs an atomic. Run after the eye's rays, before the reach
/// (`light_reach_cs`); the drawn receivers stand apart (`light_mark_drawn_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_mark_cs"))]
pub fn light_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let drawn: UVec3 = rays.query_size();
    let m = pxl_spec_core::light::MARK;
    if id.x * m >= drawn.x || id.y * m >= drawn.y {
        return;
    }
    let (s, n) = (light_of(eye), size(head));
    let (cast, footprint) = (cast_number(head), footprint(head));
    // Each pixel's tile read once: the square's, and where the footprint is
    // taken, a column past it across and a row past it down and one before
    // it, which its neighbors take (`MARK_READ`); none where a pixel met
    // nothing or lies past the drawn.
    let (cols, rows) = (m + 1, m + 2);
    let mut met = [0u32; MARK_READ];
    let mut tiles = [[0i32; 2]; MARK_READ];
    let mut k = 0;
    while k < cols * rows {
        let (x, y) = ((id.x * m + k % cols) as i32, (id.y * m + k / cols) as i32 - 1);
        let own = k % cols < m && k / cols >= 1 && k / cols <= m;
        if y >= 0 && (footprint || own) {
            let (got, tile) = met_tile(eye, rays, &s, UVec3::new(x as u32, y as u32, 0), drawn);
            met[k as usize] = u32::from(got);
            tiles[k as usize] = tile;
        }
        k += 1;
    }
    // The tile last marked, so a run of pixels on one marks it once.
    let mut last = [i32::MIN; 2];
    let mut y = 0;
    while y < m {
        let mut x = 0;
        while x < m {
            let k = ((y + 1) * cols + x) as usize;
            if met[k] != 0 {
                let tile = tiles[k];
                // [rust-gpu] By its words: an array's `!=` is an intrinsic
                // it does not lower.
                if tile[0] != last[0] || tile[1] != last[1] {
                    stand(list, n, cast, tile);
                    last = tile;
                }
                // The neighbors after it, so every pair of neighbors is taken
                // once: across, down, and the two diagonals.
                let mut j = 0;
                while j < 4 && footprint {
                    let (dx, dy) = ([1, 0, 1, 1][j], [0, 1, 1, -1][j]);
                    let q = ((y as i32 + 1 + dy) as u32 * cols + x + dx) as usize;
                    if met[q] != 0 {
                        stand_between(list, n, cast, tile, tiles[q]);
                    }
                    j += 1;
                }
            }
            x += 1;
        }
        y += 1;
    }
}

/// The pixels a thread of the mark reads (`light_mark_cs`): its square,
/// `light::MARK` a side, a column past it and a row either side.
const MARK_READ: usize =
    ((pxl_spec_core::light::MARK + 1) * (pxl_spec_core::light::MARK + 2)) as usize;

/// Whether pixel `p`'s ray met the world, and the tile it met it on. A ray
/// that met nothing leaves its first word zero (cast.rs `word`).
fn met_tile(eye: &Eye, rays: &Rays, s: &Light, p: UVec3, drawn: UVec3) -> (bool, [i32; 2]) {
    if p.x >= drawn.x || p.y >= drawn.y {
        return (false, [0; 2]);
    }
    let first: u32 = rays.read(UVec3::new(p.x, p.y, 0));
    if first == 0 || first == pxl_spec_core::ray::MISS {
        return (false, [0; 2]);
    }
    let t = f32::from_bits(rays.read(UVec3::new(p.x, p.y, 1)));
    let d = eye_ray(eye, px_tan(eye, p, drawn)).dir;
    (true, s.tile_of([d[0] * t, d[1] * t, d[2] * t]))
}

/// The tiles between `a` and `b` marked stood on, where they lie within a
/// footprint of each other (`light::FOOTPRINT_TILES`); none past it.
fn stand_between(list: &mut [u32], size: u32, cast: u32, a: [i32; 2], b: [i32; 2]) {
    let span = pxl_spec_core::light::FOOTPRINT_TILES;
    let (lo, hi) = (
        [a[0].min(b[0]), a[1].min(b[1])],
        [a[0].max(b[0]), a[1].max(b[1])],
    );
    if hi[0] - lo[0] > span || hi[1] - lo[1] > span || (lo[0] == hi[0] && lo[1] == hi[1]) {
        return;
    }
    let mut j = lo[1];
    while j <= hi[1] {
        let mut i = lo[0];
        while i <= hi[0] {
            stand(list, size, cast, [i, j]);
            i += 1;
        }
        j += 1;
    }
}

/// Tile `[i, j]`'s word among the box's tiles' words at `first` (the
/// stands, the reads), `NO_TILE` past the box.
fn tile_word(size: u32, first: u32, [i, j]: [i32; 2]) -> u32 {
    let n = pxl_spec_core::light::tiles(size) as i32;
    if i >= 0 && j >= 0 && i < n && j < n {
        first + (j * n + i) as u32
    } else {
        NO_TILE
    }
}

/// No tile's word (`tile_word`).
const NO_TILE: u32 = u32::MAX;

/// Tile `tile` of the box marked stood on in the cast numbered `cast`
/// (`light::stands_at`), none past the box.
fn stand(list: &mut [u32], size: u32, cast: u32, tile: [i32; 2]) {
    let w = tile_word(size, pxl_spec_core::light::stands_at(size), tile);
    if w != NO_TILE {
        put(list, w, cast);
    }
}

/// The drawn receivers of the shadow map, each the eye sees standing as the
/// world's do (`light_mark_cs`): the actors the host draws, a thread each
/// (`light::ACTORS_AT`); then the decor generated, a thread a slot of its
/// instances (pxl-render gpu/generated.rs), as many as are placed, where each
/// stands, as decor tells (decor.rs `Instances`). Seen where a
/// ball about its middle as wide as decor is tall and wide meets the view
/// (`eye::sees`); the tiles about its own stood on too, as many as half
/// the widest decor spans (`Light::ring`). The actors are those the scene
/// draws in the world (pxl-render gpu/renderer.rs `Cast::actors`), no more
/// than `light::ACTORS`. Run after decor is placed, before the reach.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "light_mark_drawn_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_mark_drawn_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] regions: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] instances: &[u32],
) {
    use pxl_spec_core::generated::{TALLEST, WIDEST};
    let at = pxl_spec_core::light::ACTORS_AT;
    let k = id.x as usize;
    let v = if k < pxl_spec_core::light::ACTORS {
        if k >= head.v[at].x as usize {
            return;
        }
        let p = head.v[at + 1 + k];
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
        let k = (k - pxl_spec_core::light::ACTORS) as u32;
        if k >= placed.cnt() {
            return;
        }
        let o = [eye.origin.x, eye.origin.y, eye.origin.z];
        placed.pos(k, o)
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
    let (s, n) = (light_of(eye), size(head));
    let ([i, j], cast, r) = (s.tile_of(v), cast_number(head), s.ring());
    let mut b = -r;
    while b <= r {
        let mut a = -r;
        while a <= r {
            stand(list, n, cast, [i + a, j + b]);
            a += 1;
        }
        b += 1;
    }
}

/// Which tiles of the box a receiver reads in this cast, a thread a tile,
/// after every receiver stood (`light_mark_cs`, `light_mark_drawn_cs`), before
/// the beam: one in the disk it reads (`Light::read`), within a tile and its
/// taps of a step up the way up from a tile one stands on (`Light::up`,
/// `Light::up_at`: a point that far up lies that many tiles over or one more,
/// its taps a tile either side), as decor and the actors stand on what the
/// eye meets and are no taller. One read that no cast of the epoch read is
/// the cast's to cast: its word names the cast (`light::reads_at`), and its
/// square's (`light::coarse_at`), and its held words where the eye stands
/// and no margin yet (`light::HELD_WORDS`). A tile's words its own thread's
/// alone. One claimed, the passes that cast tiles go whole
/// (`light::ARGS_WORDS`), each claim writing the same words.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_reach_cs"))]
pub fn light_reach_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] args: &mut [u32],
) {
    let side = size(head);
    let n = pxl_spec_core::light::tiles(side);
    if id.x >= n || id.y >= n {
        return;
    }
    let s = light_of(eye);
    // A tile past every slot a receiver reads holds none of its slots.
    let t = pxl_spec_core::light::TILE;
    let middle = [(id.x * t + t / 2) as i32, (id.y * t + t / 2) as i32];
    if !s.read(
        side,
        middle,
        pxl_spec_core::light::READ_PAD + t as f32 * core::f32::consts::FRAC_1_SQRT_2,
    ) {
        return;
    }
    let (cast, epoch) = (cast_number(head), epoch(head));
    let up = s.up();
    let t = [id.x as i32, id.y as i32];
    let mut read = false;
    let mut k = 0;
    while k <= up.1 && !read {
        let o = Light::up_at(up, k);
        let [first, last] = pxl_spec_core::light::READ_TILES;
        let mut b = first;
        while b <= last && !read {
            let mut a = first;
            while a <= last && !read {
                read = stood(&*list, side, cast, [t[0] - o[0] + a, t[1] - o[1] + b]);
                a += 1;
            }
            b += 1;
        }
        k += 1;
    }
    // Read, and read first in the epoch: the cast's to cast.
    let w = pxl_spec_core::light::reads_at(side) + id.y * n + id.x;
    if read && at(list, w) < epoch {
        put(list, w, cast);
        // Where the eye stands as it is cast, and no margin yet: the walks
        // that feed it lower it (`Map::told`).
        let h = s.held(side, [id.x, id.y]);
        let e = pxl_spec_core::light::HELD_EYE;
        put(
            list,
            h + pxl_spec_core::light::HELD_MARGIN,
            Margin::NONE.0.to_bits(),
        );
        put(list, h + e, eye.origin.x as u32);
        put(list, h + e + 1, eye.origin.y as u32);
        put(list, h + e + 2, eye.origin.z as u32);
        put(
            list,
            pxl_spec_core::light::coarse_square(side, [id.x, id.y]),
            cast,
        );
        let mut k = 0;
        while k < pxl_spec_core::light::ARGS_WHOLE {
            put(args, k, at(args, pxl_spec_core::light::ARGS_WHOLE + k));
            k += 1;
        }
    }
}

/// The tiles of the box an earlier cast of the epoch cast, where only the
/// eye's place moved since and the box with it, a `TILE`-aligned corner at a
/// time (pxl-render gpu/shadow.rs `aim`): a thread a tile, after the host
/// moved the reads with the box, each tile's word the word of the tile that
/// held its world texels under the corner before (`light::HEAD_CORNER_WAS`;
/// pxl-render gpu/ray/cast.rs `prepare`). A tile the box slid onto, and one
/// whose eye moved as far from where it stood when it was cast as the least
/// margin of every node the walks that fed it asked (`light::holds`,
/// `light::HELD_WORDS`), is set to none, so the reach takes it, read, as
/// read anew (`light_reach_cs`); the rest hold what their cast left, every
/// zoom their walks asked asked again, so cast anew they would meet what
/// they met. Each writes none alone, so none waits on another.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_detail_cs"))]
pub fn light_detail_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let t = pxl_spec_core::light::TILE;
    let (s, n) = (light_of(eye), size(head));
    let tiles = pxl_spec_core::light::tiles(n);
    if id.x >= tiles || id.y >= tiles {
        return;
    }
    let w = pxl_spec_core::light::reads_at(n) + id.y * tiles + id.x;
    // Where the tile lay under the corner before: one the box slid onto
    // holds none of what it read.
    let was = head.v[1];
    let c = s.corner;
    let from = [
        id.x as i32 + (c[0] - was.x) / t as i32,
        id.y as i32 + (c[1] - was.y) / t as i32,
    ];
    if from[0] < 0 || from[1] < 0 || from[0] >= tiles as i32 || from[1] >= tiles as i32 {
        put(list, w, 0);
        return;
    }
    if at(list, w) < epoch(head) {
        return;
    }
    let h = s.held(n, [id.x, id.y]);
    let margin = f32::from_bits(at(list, h + pxl_spec_core::light::HELD_MARGIN));
    let e = pxl_spec_core::light::HELD_EYE;
    let stood = [
        at(list, h + e) as i32,
        at(list, h + e + 1) as i32,
        at(list, h + e + 2) as i32,
    ];
    let stands = [eye.origin.x, eye.origin.y, eye.origin.z];
    if !pxl_spec_core::light::holds(margin, stood, stands) {
        put(list, w, 0);
    }
}

/// Whether a receiver stood on tile `tile` in the cast numbered `cast`,
/// none past the box.
fn stood(list: &[u32], size: u32, cast: u32, tile: [i32; 2]) -> bool {
    let w = tile_word(size, pxl_spec_core::light::stands_at(size), tile);
    w != NO_TILE && at(list, w) == cast
}
