// pxl-shader/sun.rs

//! The world's depth from the sun into its clipmap (pxl-spec-core `sun`),
//! every slot each frame it is cast, by the passes the eye's rays take,
//! the sun's map their observer (`Map`; observer.rs): the beam rays and
//! their cells' starts, the samples and their boxes, which the raster
//! draws, each the entry the eye's takes too (beam.rs `beam_cs`,
//! `cell_cs`, `sample_cs`; raster.rs `sample_box_cs`) handed the sun's head
//! (`Moved`); every slot taking its pick or the sky (`sun_cs`) and the
//! rest walked (`sun_miss_cs`).

use pxl_spec_core::cast::{self, Detail};
use pxl_spec_core::fill::put;
use pxl_spec_core::{at, beam, observed, raster, tally, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::{spirv, Image};

use crate::beam::{resumed, sample_put, Beamed, Leaving};
use crate::cast::{counted, listed};
use crate::eye::{eye_origin, Eye};
use crate::observed::{seen_at, tallied, Seen, Writing};
use crate::raster::{boxed, picked, NONE};
use crate::world::{Unlooked, World};

/// The world's depth from the sun, a slot of the sun's clipmap each
/// (pxl-spec-core `sun`): the slot's texel of the light box, and its ray
/// from the box's near plane along the sun's rays, its block taken off the
/// raster's pick or walked through the world's one query to the block that
/// fills (`cast::first`) as the eye's are, and the word it leaves there,
/// every frame the map is cast. Its one layer read and written in place,
/// the one format family WebGPU lets a shader do both to, as the rays' are.
type SunWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The raster's pick at each texel of the box, in the
/// box's order (pxl-spec-core `sun::Sun::texel`): its sample's number
/// (`raster::id_of`), 0 where it kept none, read as an array of one layer,
/// as the eye's pixels read theirs.
type Picks = Image!(2D, type = u32, sampled = true, arrayed = true);

/// What the sun's cast is handed beside the eye's words (pxl-spec-core
/// `sun::MOVED_HEAD`): whether it is counted (`sun::MOVED_COUNTED`), and where the
/// rays start (`sun::Starts`).
type Moved = [IVec4];

/// Where the rays start (`sun::Starts`), off the head.
fn starts(m: &Moved) -> Starts {
    Starts::of_words(|k| {
        let v = m.v[pxl_spec_core::sun::STARTS_AT + k];
        [v.x, v.y, v.z, v.w]
    })
}

fn counts(m: &Moved) -> bool {
    m[0].y != 0
}

/// The sun's cast off the eye's words (pxl-spec-core `sun::Sun::words`).
fn sun_of(eye: &Eye) -> Sun {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    Sun {
        first: v3(eye.sun[0]),
        reach: eye.sun[0].w,
        across: v3(eye.sun[1]),
        down: v3(eye.sun[2]),
        dir: v3(eye.sun[3]),
        bias: v3(eye.sun[4]),
        corner: [i(eye.sun[1]), i(eye.sun[2])],
        rim: eye.sun[3].w,
    }
}

/// The sun's map as an observer (observer.rs `Observer`), off the eye's
/// words and the head the cast is handed: the sun's cast (`sun::Sun`),
/// where its rays start (`sun::Starts`), its side, and the eye's place and
/// zooms its rays ask at, with the map's texel for their cone
/// (`Sun::detail`). Its lattice is the box's texels, a slot each: a beam ray
/// every `sun::BEAM`-th, `sun::BEAM_WINDOW` past the box on each side, a
/// sample every `sun::SAMPLE`-th slot, which wrap as the slots do.
pub(crate) struct Map {
    sun: Sun,
    starts: Starts,
    size: u32,
    eye: [i32; 3],
    detail: Detail,
}

/// No starts, for a pass that aims no ray and is handed no head
/// (`sun_cells_cs`).
const UNAIMED: Starts = Starts {
    base: [0; 2],
    across: [0.0; 3],
    down: [0.0; 3],
    at: [[0; 3]; 4],
    frac: [[0.0; 3]; 4],
};

impl Map {
    pub(crate) fn of(eye: &Eye, moved: &Moved) -> Map {
        let sun = sun_of(eye);
        let (origin, detail) = eye_origin(eye);
        Map {
            sun,
            starts: starts(moved),
            size: size(moved),
            eye: origin,
            detail: sun.detail(detail),
        }
    }

    /// Its side (`sun::MOVED_SIZE`), which its slots and its list are laid
    /// by.
    pub(crate) fn side(&self) -> u32 {
        self.size
    }

    /// Its samples across and down, a side.
    fn across(&self) -> u32 {
        self.size / pxl_spec_core::sun::SAMPLE
    }

    /// Whether box texel `p` lies where a receiver reads the map, grown by
    /// `pad` texels (`Sun::read`).
    fn read(&self, p: [i32; 2], pad: f32) -> bool {
        self.sun.read(self.size, p, pad)
    }

    /// The word a slot leaves for the walk `met` of its ray aimed `aim`.
    fn word<A>(&self, aim: &Aim, met: &cast::Met<A>) -> u32 {
        self.sun
            .word(aim.detail, pxl_spec_core::sun::depth(&self.sun, met))
    }
}

impl Observer for Map {
    type At = [u32; 2];

    fn tally(&self) -> u32 {
        tally::SUN
    }

    fn assets(&self) -> bool {
        false
    }

    fn passes(&self) -> [u32; 3] {
        [
            observed::SUN_BEAM_PASS,
            observed::SUN_SAMPLES_PASS,
            observed::SUN_MISSED_PASS,
        ]
    }

    fn apart(&self) -> bool {
        true
    }

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
        let width = aim.detail.apart * aim.detail.cone.width(0.0);
        let t = t - pxl_spec_core::sun::margin(width);
        if t > 0.0 {
            t
        } else {
            0.0
        }
    }

    fn line(&self, index: u32) -> u32 {
        pxl_spec_core::sun::line(self.size, index)
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
        let d = pxl_spec_core::sun::SAMPLE;
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
        let d = pxl_spec_core::sun::SAMPLE;
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
}

const _: () = assert!(
    beam::CELL_T == pxl_spec_core::sun::CELL_T && beam::CELL_CORNER == pxl_spec_core::sun::CELL_RAY,
    "a cell's start lies as the eye's (observer.rs `cell`)"
);

/// How far past the disk a receiver reads (`Sun::read`) each pass casts, in
/// texels: a slot the receiver's own reach (`sun::READ_PAD`); a sample the
/// slots it is one of the four about, `SAMPLE` across and down; a cell its
/// samples, a cell's half-diagonal from its middle; a beam ray the cells
/// whose window holds it, `BEAM_WINDOW` and one more beam spacings across
/// and down, from a cell's corner.
const SLOT_PAD: f32 = pxl_spec_core::sun::READ_PAD;
const SAMPLE_PAD: f32 = SLOT_PAD + pxl_spec_core::sun::SAMPLE as f32 * core::f32::consts::SQRT_2;
const CELL_PAD: f32 =
    SAMPLE_PAD + pxl_spec_core::sun::BEAM as f32 * core::f32::consts::FRAC_1_SQRT_2 + 1.0;
const BEAM_PAD: f32 = CELL_PAD
    + ((pxl_spec_core::sun::BEAM_WINDOW + 1) * pxl_spec_core::sun::BEAM) as f32
        * core::f32::consts::SQRT_2
    + pxl_spec_core::sun::BEAM as f32;

const _: () = assert!(
    pxl_spec_core::sun::RIM == 15,
    "`sun_of` reads the rim off the rays' direction's fourth word"
);

const _: () = assert!(
    pxl_spec_core::sun::TILE == 8,
    "`sun_cs` is a workgroup a tile, its threads a side of it"
);

/// The world's depth from the sun into every slot: a workgroup a tile
/// (`sun::TILE`), a thread a slot, each taking the raster's pick where its
/// own ray enters it (observer.rs `took`), or the sky where no sample about
/// it met anything (observer.rs `sky`), as the eye's pixels do (cast.rs
/// `cast_cs`), and the word it leaves there; the rest listed for their walk
/// (`sun_miss_cs`), every slot where the raster is not made yet. Only the
/// slots of the tiles the cast casts (`sun::fresh_at`): one past the disk
/// (`Sun::read`) or in a tile none marked (`sun_mark_cs`) no receiver
/// reads, and one in a tile an earlier cast of its epoch cast stands
/// (`sun::MOVED_EPOCH`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cs"))]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(descriptor_set = 0, binding = 15)] picks: &Picks,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] misses: &mut [u32],
) {
    let t = pxl_spec_core::sun::TILE;
    let m = Map::of(eye, moved);
    let n = m.side();
    let slot = UVec3::new(group.x * t + lane.x, group.y * t + lane.y, 0);
    if slot.x >= n || slot.y >= n {
        return;
    }
    let at = [slot.x, slot.y];
    let aim = m.aim(at);
    let p = m.sun.texel(n, at);
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = picks.fetch(UVec3::new(p[0], p[1], 0));
    let (took, stop, _) = observer::resolve(&m, &*list, at, &aim, kept.x);
    if took == Takes::Pick {
        let met = cast::Met {
            ends: cast::Ends::Stop,
            stop,
            visits: 0,
        };
        counted(misses, counts(moved), tally::PICKED, 1);
        // SAFETY: within the map, bounded by its size above.
        unsafe { sun.write(slot, m.word(&aim, &met)) };
        return;
    }
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = picks.fetch(UVec3::new(p[0], p[1], 0));
    let pick = kept.x;
    if pick != 0 && pick & raster::LACK == 0 {
        let (stop, _, enters) = observer::took(&m, &*list, &aim, pick);
        if enters {
            let met = cast::Met {
                ends: cast::Ends::Stop,
                stop,
                visits: 0,
            };
            counted(misses, counts(moved), tally::PICKED, 1);
            let s = sun_of(eye);
            // SAFETY: within the map, bounded by its size above.
            unsafe { sun.write(slot, m.word(&aim, &met)) };
            return;
        }
    }
    if pick == 0 && observer::sky(&m, &*list, at) {
        // SAFETY: within the map, bounded by its size above.
        unsafe { sun.write(slot, m.sun.word(aim.detail, 1.0)) };
        return;
    }
    // Listed for its walk again as the eye's pixels are (cast.rs `listed`):
    // the list holds every slot, so none finds it full.
    counted(misses, counts(moved), tally::WALKED, 1);
    let at = listed(misses, size.x * size.y, [0; 2]);
    put(
        list,
        pxl_spec_core::sun::misses_at(n) + at,
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
pub fn sun_miss_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let m = Map::of(eye, moved);
    let n = m.side();
    let i = (group.y * pxl_spec_core::ray::MISS_ACROSS + group.x) * pxl_spec_core::ray::MISS_LANES
        + lane.x;
    if i >= at(list, pxl_spec_core::sun::misses_count(n)) {
        return;
    }
    let w = at(list, pxl_spec_core::sun::misses_at(n) + i);
    let slot = UVec3::new(w & 0xffff, w >> 16, 0);
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let (p, o, ray, detail) = sun_slot(eye, moved, size.x, slot);
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_MISSED_PASS);
    let met = sun_resumed(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            blocks: false,
        },
        list,
        size.x,
        p,
        o,
        &ray,
        detail,
    );
    tallied(seen, tally::SUN, tally::MISS, met.visits);
    let s = sun_of(eye);
    // SAFETY: a slot `sun_cs` listed, within the map.
    unsafe { sun.write(slot, m.word(&aim, &met)) };
}

/// The world's receivers of the sun's map: the drawn pixels' rays (cast.rs
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
    let d = pxl_spec_core::sun::SAMPLE;
    if id.x >= size.x / d || id.y >= size.x / d {
        return;
    }
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let s = sun_of(eye);
    let p = s.texel(size.x, [id.x * d, id.y * d]);
    let (origin, detail) = eye_origin(eye);
    let mut detail = s.detail(detail);
    let (o, ray, from) = pxl_spec_core::sun::ray(origin, &s, &starts(moved), p);
    detail.eye = from;
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_SAMPLES_PASS);
    let met = sun_resumed(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            blocks: false,
        },
        list,
        size.x,
        p,
        o,
        &ray,
        detail,
    );
    tallied(seen, tally::SUN, tally::SAMPLE, met.visits);
    let index = id.y * (size.x / d) + id.x;
    sample_put(list, pxl_spec_core::sun::sample(size.x, index), &met);
}

/// Each sample of the sun's map (`sun_sample_cs`), a
/// thread each, written as a box for the raster's draw by the eye's writer
/// (raster.rs `boxed`), the one across and up before it, from where box
/// texel (0, 0)'s ray starts; the sun's has no box about its eye (pxl-spec-core
/// `sun::SAMPLE`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_boxes_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_boxes_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let size: UVec3 = sun.query_size();
    let d = pxl_spec_core::sun::SAMPLE;
    // [naga] The eye's vec2 of u32s read, so the module holds the type
    // naga's SPIR-V reader splits an arrayed image's layer off by (`Eye`).
    if id.x >= size.x / d || id.y >= size.x / d || eye.range.x > eye.range.y {
        return;
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
    boxed(
        list,
        at(id.x, id.y),
        before,
        origin,
        id.y * n + id.x,
        (false, [0.0; 2]),
        boxes,
        args,
    );
}

/// Each beam ray of the sun's map (pxl-spec-core `sun::BEAM`), a thread
/// each, `sun::beam_across` a side, `sun::BEAM_WINDOW` of them past the box
/// on each side: its ray from where its texel of the box lies on the near
/// plane (`sun::Starts`), walked from the world's root to the first block
/// that fills or lacks or the first split narrower than `2√2` of the
/// beam's spacing (`cast::Until::Beam`, `cast::Detail::apart`), its line
/// left for the texels about it (`Leaving`) and how far they may start
/// from it, its stop less `sun::margin`.
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
    let across = pxl_spec_core::sun::beam_across(size.x);
    if id.x >= across || id.y >= across {
        return;
    }
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let s = sun_of(eye);
    let (b, w) = (
        pxl_spec_core::sun::BEAM as i32,
        pxl_spec_core::sun::BEAM_WINDOW as i32,
    );
    // Its texel of the box, which may lie past it.
    let p = [(id.x as i32 - w) * b, (id.y as i32 - w) * b];
    let o = starts(moved).start([
        s.corner[0].wrapping_add(p[0]),
        s.corner[1].wrapping_add(p[1]),
    ]);
    let (origin, detail) = eye_origin(eye);
    let mut detail = s.detail(detail);
    let q3 = (1u32 << pxl_spec_core::FRAC_W) as f32;
    detail.eye = [
        origin[0].wrapping_sub(o[0]) as f32 / q3,
        origin[1].wrapping_sub(o[1]) as f32 / q3,
        origin[2].wrapping_sub(o[2]) as f32 / q3,
    ];
    let width = detail.apart * detail.cone.width(0.0);
    let ray = cast::Ray::new(s.dir, s.reach);
    let base = pxl_spec_core::sun::line(size.x, id.y * across + id.x);
    let k = base + pxl_spec_core::sun::LINE_ORIGIN;
    put(list, k, o[0] as u32);
    put(list, k + 1, o[1] as u32);
    put(list, k + 2, o[2] as u32);
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_BEAM_PASS);
    let met = cast::rooted(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            blocks: false,
        },
        o,
        &ray,
        detail,
        Until::Beam,
        &mut Leaving(list, base),
    );
    tallied(seen, tally::SUN, tally::BEAM, met.visits);
    let t = met.stop.t - pxl_spec_core::sun::margin(width);
    put(
        list,
        base + beam::LINE_T,
        if t > 0.0 { t } else { 0.0 }.to_bits(),
    );
}

/// Where each cell of the sun's map starts (pxl-spec-core `sun::cell`), a
/// thread a cell, `sun::BEAM` texels a side: the least of the beam rays
/// within `sun::BEAM_WINDOW` of its corners, and that ray, whose line its
/// texels resume from.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cells_cs"))]
pub fn sun_cells_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let drawn: UVec3 = rays.query_size();
    let m = pxl_spec_core::sun::MARK;
    if id.x * m >= drawn.x || id.y * m >= drawn.y {
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
                let d = eye_ray(eye, pixel_tan(eye, p, drawn)).direction;
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
}

/// Tile `[i, j]` of the box and the tiles beside it marked read in the cast
/// numbered `cast`, none past the box; each that no cast since `epoch`
/// marked, which the mark it overwrites tells, cast in it
/// (`sun::fresh_at`). Of two threads marking one, whichever reads the
/// other's mark leaves its fresh word to the other, which writes it.
fn mark_about(list: &mut [u32], size: u32, [cast, epoch]: [u32; 2], [i, j]: [i32; 2]) {
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

/// The drawn receivers of the sun's map, each the eye sees standing as the
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
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] regions: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] instances: &[u32],
) {
    use pxl_spec_core::scatter::{TALLEST, WIDEST};
    let head = pxl_spec_core::sun::ACTORS_AT;
    let k = id.x as usize;
    let v = if k < pxl_spec_core::sun::ACTORS {
        if k >= moved.v[head].x as usize {
            return;
        }
        let p = moved.v[head + 1 + k];
        [
            f32::from_bits(p.x as u32),
            f32::from_bits(p.y as u32),
            f32::from_bits(p.z as u32),
        ]
    } else {
        let placed = Instances {
            params,
            regions,
            instances,
        };
        let k = (k - pxl_spec_core::sun::ACTORS) as u32;
        if k >= placed.count() {
            return;
        }
        let o = [eye.origin.x, eye.origin.y, eye.origin.z];
        placed.stands(k, o)
    };
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let middle = [v[0], v[1], v[2] + TALLEST as f32 / 2.0];
    let axes = [v3(eye.forward_far), v3(eye.right_tau), v3(eye.up)];
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
            stand(list, n, cast, [i + a, j + b]);
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
/// square's (`sun::coarse_at`). A tile's word its own thread's alone. One
/// claimed, the passes that cast tiles go whole (`sun::ARGS_WORDS`), each
/// claim writing the same words.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_reach_cs"))]
pub fn sun_reach_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] args: &mut [u32],
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
        side,
        middle,
        pxl_spec_core::sun::READ_PAD + t as f32 * core::f32::consts::FRAC_1_SQRT_2,
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
                read = stood(&*list, side, cast, [t[0] - o[0] + a, t[1] - o[1] + b]);
                a += 1;
            }
            b += 1;
        }
        k += 1;
    }
    // Read, and read first in the epoch: the cast's to cast.
    let w = pxl_spec_core::sun::reads_at(side) + id.y * n + id.x;
    if read && at(list, w) < epoch {
        put(list, w, cast);
        put(
            list,
            pxl_spec_core::sun::coarse_square(side, [id.x, id.y]),
            cast,
        );
        let mut k = 0;
        while k < pxl_spec_core::sun::ARGS_WHOLE {
            put(args, k, at(args, pxl_spec_core::sun::ARGS_WHOLE + k));
            k += 1;
        }
    }
}

/// Whether a receiver stood on tile `tile` in the cast numbered `cast`,
/// none past the box.
fn stood(list: &[u32], size: u32, cast: u32, tile: [i32; 2]) -> bool {
    let w = tile_word(size, pxl_spec_core::sun::stands_at(size), tile);
    w != NO_TILE && at(list, w) == cast
}
