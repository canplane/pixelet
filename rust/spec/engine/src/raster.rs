// pxl-engine/raster.rs

//! gpu-portable: the voxels drawn as boxes, so the rasterizer keeps each drawn pixel's cell
//! (pxl-render raster.wgsl, cast.wgsl `cast_cs`, light.wgsl, gpu/ray/raster.rs). The eye's
//! voxels and the light's are each one record's, kept across frames (`record`), every held
//! one in view drawn each frame the observer casts (pxl-render record_draw.wgsl,
//! light_record.wgsl). A box is a node as its ray stops on it (`cast::asset_box`). It draws, of its faces turned
//! to the eye, those its rays entered it across (`faces`), a lack's every face turned to it,
//! and a face keeps a pixel only where the pixel's own ray enters the box across that face's
//! axis (`cast::entry`'s test; raster.wgsl `fs`): a face some ray reached through air is one
//! the pixel's may show where it enters across it. A ray enters a box through one face, so
//! what is kept lies where the ray enters, at the depth the raster draws it, and the depth
//! test keeps the nearest box the pixel's ray enters, a fresh voxel's before any stale one's
//! (`STALE`), leaving its voxel's number (`id_of`): its cell where a hit, and where a lack, a
//! node it walks itself.

use crate::node_cache::ENTRIES;

/// The draw's indirect args, a buffer of their own (`ARGS_WORDS`):
/// `ARGS_VERTICES`, `ARGS_INSTANCES`, then two words the draw reads as 0;
/// then the dispatch over the boxes that asks what lies about each one's
/// face (pxl-render face.wgsl `face_cs`), `FACES_LANES` boxes a workgroup
/// (`ARGS_FACES`), its workgroups raised by the box that opens each, the
/// other two 1; then the first word of the boxes' asks among the beam rays'
/// words (`ARGS_ASKS`, `beam::ASK_WORDS` a box, `beam::Grid::asks_at`);
/// then the boxes counted as they are written (`ARGS_CNT`), of which
/// `ARGS_INSTANCES` is the part the boxes' room holds.
/// The eye's boxes are the drawable entries in view, `record::EYE_DRAWS` at most; the
/// light's those on its box, `record::LIGHT_DRAWS` at most. The faces'
/// asks are a sample's at most, the samples `param::beam::SAMPLE` a camera grid cell a
/// side, about 270 thousand on a 16:9 view whatever its pixels; one dispatch's 65535
/// workgroups hold 4.2M.
/// The draw's args are its own words alone; the faces' asks', a voxel's face each, laid the
/// same way at `ASKED_AT`, a storage binding's alignment past them, which the faces' pass binds
/// alone.
/// Then the draws' buffer, a word a box (`draw_word`): its voxel's number (`id_of`), with
/// `EYED` where the box lies about the eye (`is_eyed`) and `STALE` where its voxel is not the
/// node the observer's walk stops on there (`record::fresh`, `record::light_fresh`). The box
/// itself is its entry's, which the raster's vertex stage reads by the voxel's slot: its node's
/// planes from the observer's origin, the cell its asset keeps inset (`cast::asset_box`), and
/// its faces drawn, a bit a side (`faces`), those an eye's ray asked about
/// (`record::ASKED`), every one some ray entered for the light (`record::FACES`).
pub const ARGS_VERTICES: u32 = 0;
pub const ARGS_INSTANCES: u32 = 1;
pub const ARGS_FACES: u32 = 4;
pub const ARGS_ASKS: u32 = 7;
pub const ARGS_CNT: u32 = 8;
pub const ARGS_WORDS: u32 = 9;
pub const ASKED_AT: u32 = crate::STORAGE_ALIGN_WORDS;
/// The faces' asks the holes' stops open past the samples' (`record`), laid as theirs.
pub const HIT_ASKED_AT: u32 = 2 * crate::STORAGE_ALIGN_WORDS;
/// The args' words, the draw's and both faces' asks'.
pub const WORDS: u32 = HIT_ASKED_AT + ARGS_WORDS;
pub const FACES_LANES: u32 = 64;
pub const EYED: u32 = 1 << 29;
pub const STALE: u32 = 1 << 30;
/// A draw word's voxel's number (`id_of`), past `EYED` and `STALE`.
pub const ID_MASK: u32 = LACK | (EYED - 1);
const _: () = assert!(
    ENTRIES < EYED,
    "a voxel's number lies under the draw's bits"
);

/// The word a box is drawn by: voxel `voxel`'s number, a lack's where `lacks`, `EYED` where
/// `eyed`, `STALE` where not `fresh`.
pub const fn draw_word(voxel: u32, lacks: bool, eyed: bool, fresh: bool) -> u32 {
    id_of(voxel, lacks) | if eyed { EYED } else { 0 } | if fresh { 0 } else { STALE }
}
const _: () = assert!(
    ARGS_WORDS <= ASKED_AT
        && ASKED_AT + ARGS_WORDS <= HIT_ASKED_AT
        && (ASKED_AT * 4).is_multiple_of(256)
        && (HIT_ASKED_AT * 4).is_multiple_of(256)
);

/// A voxel's faces drawn, a bit a side, six: each face some ray entered it through
/// (`ray::face`, twice its axis and one more where the ray ran up the axis, entering the low
/// side), a side a ray reached through air from outside it. Not a bit an axis: the other face
/// of an axis is one no ray may have reached, and may lie against what fills.
pub fn faces(faces: impl IntoIterator<Item = u32>) -> u32 {
    faces.into_iter().fold(0, |m, f| m | 1 << f)
}

/// Whether a pixel takes a voxel it enters through face `face` (`ray::face`): a face drawn
/// there (`faces`).
pub fn accepts(faces: u32, face: u32) -> bool {
    faces >> face & 1 != 0
}

/// A box's corners drawn: its three faces turned to the eye, one an axis,
/// two triangles each (`corner`), a face not drawn left a point.
pub const VERTICES: u32 = 18;

/// A voxel's number as the raster leaves it at a pixel or a slot: its record's slot
/// (`record::entry_at`), plus one, so 0 is none, and `LACK` where it lacks.
pub const LACK: u32 = 1 << 31;

pub const fn id_of(voxel: u32, lacks: bool) -> u32 {
    (voxel + 1) | if lacks { LACK } else { 0 }
}

/// The voxel an id names (`id_of`), none for 0.
pub const fn voxel_of(id: u32) -> u32 {
    (id & ID_MASK & !LACK) - 1
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
