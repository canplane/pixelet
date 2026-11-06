// pxl-engine/raster.rs

//! gpu-portable: the voxels drawn as boxes, so the rasterizer keeps each drawn pixel's cell
//! (pxl-render raster.wgsl, cast.wgsl `cast_cs`, light.wgsl, gpu/ray/raster.rs). The eye's
//! voxels are the record's, kept across frames (`record`), every held one in view drawn each
//! frame (pxl-render record_draw.wgsl); the light's are its samples' of one cast, drawn once
//! where the samples of one node meet in the cast's table of voxels (`KEYS`, boxes.wgsl), and
//! by a sample alone where its node is in no slot of it, a voxel twice then but never none.
//! A box is a node as its ray stops on it (`cast::asset_box`). It draws, of its faces turned
//! to the eye, those its rays entered it across (`faces`), a lack's every face turned to it,
//! and a face keeps a pixel only where the pixel's own ray enters the box across that face's
//! axis (`cast::entry`'s test; raster.wgsl `fs`): a face some ray reached through air is one
//! the pixel's may show where it enters across it. A ray enters a box through one face, so
//! what is kept lies where the ray enters, at the depth the raster draws it, and the depth
//! test keeps the nearest box the pixel's ray enters, a fresh voxel's before any stale one's
//! (`STALE`), leaving its voxel's number (`id_of`): its cell where a hit, and where a lack, a
//! node it walks itself.

use crate::beam;

/// The draw's indirect args, a buffer of their own (`ARGS_WORDS`):
/// `ARGS_VERTICES`, `ARGS_INSTANCES`, then two words the draw reads as 0;
/// then the dispatch over the boxes that asks what lies about each one's
/// face (pxl-render face.wgsl `face_cs`), `FACES_LANES` boxes a workgroup
/// (`ARGS_FACES`), its workgroups raised by the box that opens each, the
/// other two 1; then the first word of the boxes' asks among the beam rays'
/// words (`ARGS_ASKS`, `beam::ASK_WORDS` a box, `beam::Grid::asks_at`);
/// then the boxes counted as they are written (`ARGS_CNT`), of which
/// `ARGS_INSTANCES` is the part the boxes' room holds.
/// The eye's boxes are the record's held voxels in view, `record::ENTRIES` at most; the
/// light's a sample's at most, its samples `param::light::SAMPLE` texels apart. The faces'
/// asks are a sample's at most, the samples `param::beam::SAMPLE` a camera grid cell a
/// side, about 270 thousand on a 16:9 view whatever its pixels; one dispatch's 65535
/// workgroups hold 4.2M.
/// The draw's args are its own words alone; the faces' asks', a voxel's face each, laid the
/// same way at `ASKED_AT`, a storage binding's alignment past them, which the faces' pass binds
/// alone.
/// Then the boxes' buffer, `BOX_WORDS` a box: its planes in units from the
/// eye's origin, lowest then highest, f32 bits, its voxel's number
/// (`id_of`), and its faces drawn, a bit an axis (`faces`), with `EYED`
/// where the box lies about the eye (`is_eyed`) and `STALE` where its voxel
/// is not the node the eye's walk stops on (`record::fresh`); past the light's,
/// the cast's table of voxels (`keys_at`).
pub const ARGS_VERTICES: u32 = 0;
pub const ARGS_INSTANCES: u32 = 1;
pub const ARGS_FACES: u32 = 4;
pub const ARGS_ASKS: u32 = 7;
pub const ARGS_CNT: u32 = 8;
pub const ARGS_WORDS: u32 = 9;
pub const ASKED_AT: u32 = 64;
/// The faces' asks the record's stops open past the samples' (`record`), laid as theirs.
pub const STOP_ASKED_AT: u32 = 128;
/// The hits whose stop met its key's claim (`record`, `beam::Grid::defers_at`): the dispatch
/// over them, `beam::HIT_LANES` a workgroup, raised by the one that opens each, the other two
/// 1 (`DEFERRED_ARGS`), then their count (`DEFERRED_CNT`).
pub const DEFERRED_AT: u32 = 192;
pub const DEFERRED_ARGS: u32 = DEFERRED_AT;
pub const DEFERRED_CNT: u32 = DEFERRED_AT + 3;
pub const DEFERRED_WORDS: u32 = 4;
pub const FACES_LANES: u32 = 64;
pub const BOX_LO: u32 = 0;
pub const BOX_HI: u32 = 3;
pub const BOX_ID: u32 = 6;
pub const BOX_FACES: u32 = 7;
pub const BOX_WORDS: u32 = 8;
pub const EYED: u32 = 1 << 8;
pub const STALE: u32 = 1 << 9;
const _: () = assert!(
    ARGS_WORDS <= ASKED_AT
        && ASKED_AT + ARGS_WORDS <= STOP_ASKED_AT
        && (ASKED_AT * 4).is_multiple_of(256)
        && (STOP_ASKED_AT * 4).is_multiple_of(256)
        && STOP_ASKED_AT + ARGS_WORDS <= DEFERRED_AT
);

/// The light's cast's table of voxels, past its boxes: `KEYS` slots a sample, two words a slot,
/// the number plus one of the sample that claimed it, 0 where free, then its voxel's
/// number plus one once that sample wrote it. Only a sample whose node no sample before
/// it across or up stopped on claims a slot; every sample then finds its node's
/// (pxl-render boxes.wgsl). Cleared before every cast that writes boxes, so it holds that
/// cast's voxels alone.
pub const KEYS: u32 = 2;
pub const KEY_WORDS: u32 = 2;

/// The words the boxes' buffer takes for `samples` samples, each a box at
/// most, and the table past them (`keys_at`).
pub const fn words(samples: u32) -> u32 {
    samples * (BOX_WORDS + KEYS * KEY_WORDS)
}

/// The first word of the table of voxels for `samples` samples.
pub const fn keys_at(samples: u32) -> u32 {
    samples * BOX_WORDS
}

/// What a voxel is told apart by, the sample's at word `at` of `words`
/// (`beam::SAMPLE_NODE`): its node, its zoom, corner and roots, and the band its
/// ray asked (the light's, `light::Light::band`; the eye's 0). A stopped cell's
/// answer is the world's query's for its node from what its parent handed it, the
/// zooms asked aside (pxl-render world_split.wgsl `world_node`: a node the query
/// stops in is answered by `world_cell`, which takes no zoom asked, and a split
/// coarser than asked answers a cell only void, which no ray stops in), and the
/// zooms a node is asked at are its key's alone: the eye's off its distance from
/// the eye every eye's ray starts at (pxl-render walk.wgsl `walk`), the light's its
/// band. So samples of one key hold one answer and one box.
pub fn key(words: &[u32], at: usize, band: u32) -> ([u32; beam::NODE_WORDS as usize], u32) {
    let k = at + beam::SAMPLE_NODE as usize;
    (core::array::from_fn(|i| words[k + i]), band)
}

/// A voxel's faces drawn, a bit an axis: each axis one of its samples entered it
/// across (`beam::SAMPLE_AXIS`), the face a ray entering across it reaches through
/// air.
pub fn faces(axes: impl IntoIterator<Item = u32>) -> u32 {
    axes.into_iter().fold(0, |m, a| m | 1 << a)
}

/// Whether a pixel takes a voxel it enters across `axis`: a face drawn there
/// (`faces`).
pub fn accepts(faces: u32, axis: u32) -> bool {
    faces >> axis & 1 != 0
}

/// A box's corners drawn: its three faces turned to the eye, one an axis,
/// two triangles each (`corner`), a face not drawn left a point.
pub const VERTICES: u32 = 18;

/// A voxel's number as the raster leaves it at a pixel: the eye's its record's slot
/// (`record::entry_at`), the light's its place among the cast's voxels (`light::voxels_at`),
/// its box's, plus one, so 0 is none, and `LACK` where it lacks.
pub const LACK: u32 = 1 << 31;

pub const fn id_of(voxel: u32, lacks: bool) -> u32 {
    (voxel + 1) | if lacks { LACK } else { 0 }
}

/// The voxel an id names (`id_of`), none for 0.
pub const fn voxel_of(id: u32) -> u32 {
    (id & !LACK) - 1
}

/// The raster's reversed depth: a face at view depth `w` drawn at `NEAR /
/// w`, nearer greater, nothing nearer than `NEAR`, a Q3 step. A box whose
/// nearest point lies within `is_eyed`'s reach of the eye may be cut by it,
/// and is drawn over the whole screen instead, at the nearest depth, a
/// lack's nearer still (raster.wgsl): every pixel then takes it, and one
/// whose ray does not enter it walks, never falling to what lies behind.
pub const NEAR: f32 = 1.0 / (1 << crate::FRAC_W) as f32;

/// What a stale voxel's depth is drawn times (`BOX_FACES`'s `STALE`): 2^-64, so its every
/// fragment lies behind every fresh one's, the nearest of which at `REACH_MOST` is still
/// `NEAR / REACH_MOST` and so over it, and the stale ones keep their order among themselves,
/// an f32's relative precision being the same at every exponent its normals reach.
pub const STALE_DEPTH: f32 = 1.0 / (1u64 << 32) as f32 / (1u64 << 32) as f32;

/// The farthest an eye's ray reaches for which every fresh box lies before every stale one
/// (`STALE_DEPTH`): 2^55 units, 2^31 times once round the world.
pub const REACH_MOST: f32 = (1u64 << 55) as f32;
const _: () = assert!(
    NEAR / REACH_MOST > STALE_DEPTH,
    "a fresh box at the farthest reach lies before every stale one"
);

/// Whether the box `lo`..`hi`, in units from the eye, lies so near it that
/// the raster's near plane may cut it (`NEAR`), `tan` the tangents of the
/// screen's half-angles: its nearest point within `NEAR` at the screen's
/// corner, twice over. A lack's box the eye is in is that too.
pub fn is_eyed(lo: [f32; 3], hi: [f32; 3], tan: [f32; 2]) -> bool {
    let near = |a: usize| {
        let v = if lo[a] > 0.0 {
            lo[a]
        } else if hi[a] < 0.0 {
            hi[a]
        } else {
            0.0
        };
        v * v
    };
    let k = 2.0 * NEAR;
    near(0) + near(1) + near(2) < k * k * (1.0 + tan[0] * tan[0] + tan[1] * tan[1])
}

/// Whether the eye's `right`, `up` and `forward` turn the other way from
/// the one `corner` winds a front face for: right × up along forward.
pub fn is_flipped(right: [f32; 3], up: [f32; 3], forward: [f32; 3]) -> bool {
    let x = [
        right[1] * up[2] - right[2] * up[1],
        right[2] * up[0] - right[0] * up[2],
        right[0] * up[1] - right[1] * up[0],
    ];
    x[0] * forward[0] + x[1] * forward[1] + x[2] * forward[2] > 0.0
}

/// A box's corner `k` of a cube's 36 (raster.wgsl, where build.rs writes
/// them): two triangles a face, the faces in their order across X, then
/// Y, then Z, low side first, each wound counter-clockwise seen from
/// outside, so a face turned to the eye is wound counter-clockwise on the
/// screen (`facing_face_ccw`) and the
/// raster culls the faces turned away; each triangle wound the other way
/// where `flipped`.
pub fn corner(k: u32, flipped: bool) -> [u32; 3] {
    let face = k / 6;
    // A triangle's second and third corners swapped turn it the other way.
    let j = match (k % 6, flipped) {
        (1, true) => 2,
        (2, true) => 1,
        (4, true) => 5,
        (5, true) => 4,
        (j, _) => j,
    };
    let t = match j {
        0 | 3 => 0,
        1 => 1,
        2 | 4 => 2,
        _ => 3,
    };
    let (a, high) = (face / 2, face % 2);
    // The quad's corners in (u, v), u along the next axis and v the one
    // after it: counter-clockwise about the face's outward normal on the
    // high side, clockwise on the low, whose normal is the other way.
    let (u, v) = match t {
        0 => (0, 0),
        1 => (1, 0),
        2 => (1, 1),
        _ => (0, 1),
    };
    let (u, v) = if high == 1 { (u, v) } else { (v, u) };
    let mut c = [0u32; 3];
    c[a as usize] = high;
    c[((a + 1) % 3) as usize] = u;
    c[((a + 2) % 3) as usize] = v;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every face of a box the eye stands outside of and before, projected
    /// as the raster projects it (raster.wgsl `vs`), is wound
    /// counter-clockwise on the screen where its outward normal is turned
    /// to the eye, clockwise where it is turned away: a front face is kept.
    #[test]
    fn facing_face_ccw() {
        // An eye looking north, then one turned and pitched down.
        let views: [[[f32; 3]; 3]; 2] = [
            [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            [
                [0.8, -0.6, 0.0],
                [0.36, 0.48, 0.8],
                [-0.48, -0.64, 0.6].map(|v: f32| -v),
            ],
        ];
        // Each mirrored too, as an eye whose axes turn the other way.
        let views = views
            .into_iter()
            .flat_map(|[r, u, f]| [[r, u, f], [r.map(|v| -v), u, f]]);
        for [r, u, f] in views {
            let flip = is_flipped(r, u, f);
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            for at in [[-0.5f32, 4.0, -0.7], [2.0, 6.0, 1.0], [-3.0, 3.0, 2.5]] {
                let p = |c: [u32; 3]| {
                    let q = [
                        at[0] + c[0] as f32,
                        at[1] + c[1] as f32,
                        at[2] + c[2] as f32,
                    ];
                    let w = dot(q, f);
                    [dot(q, r) / w, dot(q, u) / w, w]
                };
                for face in 0..6u32 {
                    let [a, b, c] = [0, 1, 2].map(|j| p(corner(face * 6 + j, flip)));
                    if a[2] <= 0.0 || b[2] <= 0.0 || c[2] <= 0.0 {
                        continue;
                    }
                    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                    // The face's outward normal and a point on it.
                    let axis = (face / 2) as usize;
                    let mut n = [0.0f32; 3];
                    n[axis] = if face % 2 == 1 { 1.0 } else { -1.0 };
                    let mut on = at;
                    on[axis] += (face % 2) as f32;
                    let turned = dot(n, on) < 0.0;
                    assert_eq!(area > 0.0, turned, "{at:?} face {face}, flipped {flip}");
                }
            }
        }
    }

    /// An id names its voxel again, and says whether it lacks.
    #[test]
    fn id_names_sample() {
        for (s, lacks) in [(0, false), (7, true), ((1 << 25) - 1, false)] {
            let id = id_of(s, lacks);
            assert_ne!(id, 0);
            assert_eq!(voxel_of(id), s);
            assert_eq!(id & LACK != 0, lacks);
        }
    }
}
