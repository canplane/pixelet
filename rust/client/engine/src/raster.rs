// pxl-engine/raster.rs

//! wgsl-port: the voxels drawn as boxes, so that the rasterizer leaves at each drawn pixel
//! the cell that pixel shows (pxl-render raster.wgsl, ray/raster.rs). The eye's voxels
//! and the light's are each kept in a record across frames (`record`), and every held voxel
//! in view is drawn each frame its observer casts (pxl-render record_draw.wgsl). A box is a
//! node as its ray stops on it (`cast::asset_box`).
//!
//! Of its faces turned to the eye, a box draws those its rays entered it through (`faces`);
//! a lack, a node whose data is not held, draws every face turned to the eye. A face keeps a
//! pixel only where the pixel's own ray enters the box across that face's axis
//! (`cast::entry`'s test; raster.wgsl `fs`): a face that some ray reached through air may show
//! at a pixel whose ray enters across it. A ray enters a box through one face, so what is
//! kept lies where the ray enters, at the depth the raster draws it. The depth test then keeps
//! the nearest box the pixel's ray enters, for the light a fresh voxel before any stale one
//! (`STALE`), and leaves that voxel's number at the pixel (`id_of`). For a hit the number names the cell;
//! for a lack, the pixel walks the node itself.

use crate::node_cache::ENTRIES;
use pxl_spec::space::coord::q3::FRAC_W;

/// The draw's indirect arguments, in a buffer of their own (`ARGS_WORDS` words). First come
/// `ARGS_VERTICES` and `ARGS_INSTANCES`, then two words the draw reads as 0. Next is the
/// dispatch over the boxes that asks what lies around each box's face (pxl-render face.wgsl
/// `face_cs`), `FACES_LANES` boxes a workgroup (`ARGS_FACES`): its workgroup count is raised
/// by the box that opens each workgroup, and its other two counts are 1. Then comes the first
/// word of the boxes' asks among the beam rays' words (`ARGS_ASKS`, `beam::ASK_WORDS` a box,
/// `beam::Grid::asks_at`), and last the boxes counted as they are written (`ARGS_CNT`), of
/// which `ARGS_INSTANCES` is the part the boxes' room holds.
///
/// The eye's boxes are its fresh drawable entries in view, at most `record::EYE_DRAWS`; the
/// light's are those on its box, at most `record::LIGHT_DRAWS`. The eye's draw reads these
/// arguments. The light's draws every place of its draws' buffer, and its arguments keep the
/// boxes' count alone. The faces' asks are at most
/// one a sample, with `param::beam::SAMPLE` samples a camera grid cell a side: about 270
/// thousand on a 16:9 view, whatever its pixels, where one dispatch's 65535 workgroups hold
/// 4.2 million.
///
/// The draw's arguments hold only its own words. The faces' asks, one for each voxel's face,
/// are laid out the same way at `ASKED_AT`, a storage binding's alignment past them, so the
/// faces' pass can bind them alone.
///
/// The draws' buffer follows, one word a box (`draw_word`): the voxel's number (`id_of`), with
/// `EYED` where the box lies around the eye (`is_eyed`) and `STALE` where the voxel is not the
/// node the light's walk stops on there (`record::light_fresh`). The box
/// itself belongs to the entry, which the raster's vertex stage reads by the voxel's slot: the
/// node's planes from the observer's origin, the cell its asset keeps inset
/// (`cast::asset_box`), and the faces it draws, one bit a side (`faces`). The eye draws the
/// faces an eye's ray asked about (`record::ASKED`); the light draws every face some ray
/// entered (`record::FACES`).
pub const ARGS_VERTICES: u32 = 0;
pub const ARGS_INSTANCES: u32 = 1;
pub const ARGS_FACES: u32 = 4;
pub const ARGS_ASKS: u32 = 7;
pub const ARGS_CNT: u32 = 8;
pub const ARGS_WORDS: u32 = 9;
pub const ASKED_AT: u32 = crate::STORAGE_ALIGN_WORDS;
/// Where the faces' asks opened by the holes' stops begin, past the samples' asks (`record`),
/// laid out as those are.
pub const HIT_ASKED_AT: u32 = 2 * crate::STORAGE_ALIGN_WORDS;
/// The arguments' words: the draw's and both sets of faces' asks.
pub const WORDS: u32 = HIT_ASKED_AT + ARGS_WORDS;
pub const FACES_LANES: u32 = 64;
pub const EYED: u32 = 1 << 29;
pub const STALE: u32 = 1 << 30;
/// The bits of a draw word that hold the voxel's number (`id_of`): every bit but `EYED` and
/// `STALE`.
pub const ID_MASK: u32 = LACK | (EYED - 1);
const _: () = assert!(
    ENTRIES < EYED,
    "a voxel's number lies under the draw's bits"
);

pub const fn draw_word(voxel: u32, lacks: bool, eyed: bool, fresh: bool) -> u32 {
    id_of(voxel, lacks) | if eyed { EYED } else { 0 } | if fresh { 0 } else { STALE }
}
const _: () = assert!(
    ARGS_WORDS <= ASKED_AT
        && ASKED_AT + ARGS_WORDS <= HIT_ASKED_AT
        && (ASKED_AT * 4).is_multiple_of(256)
        && (HIT_ASKED_AT * 4).is_multiple_of(256)
);

/// A voxel's drawn faces, one bit for each of its six sides: each face some ray entered it
/// through (`ray::face`: twice the axis, plus one where the ray ran up the axis and so entered
/// the low side). Such a side is one a ray reached through air from outside. The bits are a
/// side each, not an axis each, because no ray may have reached the other face of an axis,
/// and that face may lie against something solid.
pub fn faces(faces: impl IntoIterator<Item = u32>) -> u32 {
    faces.into_iter().fold(0, |m, f| m | 1 << f)
}

/// Whether a pixel takes a voxel it enters through face `face` (`ray::face`): only where that
/// face is drawn (`faces`).
pub fn accepts(faces: u32, face: u32) -> bool {
    faces >> face & 1 != 0
}

/// The corners a box is drawn with: its three faces turned to the eye, one on each axis, two
/// triangles each (`corner`). A face that is not drawn collapses to a point.
pub const VERTICES: u32 = 18;

/// A voxel's number, as the raster leaves it at a pixel or a slot: its slot in the record
/// (`node_cache::entry_at`) plus one, so that 0 means none, with `LACK` set where the voxel
/// is a lack.
pub const LACK: u32 = 1 << 31;

pub const fn id_of(voxel: u32, lacks: bool) -> u32 {
    (voxel + 1) | if lacks { LACK } else { 0 }
}

/// The voxel an id names (`id_of`); 0 names none.
pub const fn voxel_of(id: u32) -> u32 {
    (id & ID_MASK & !LACK) - 1
}

/// The raster's reversed depth: a face at view depth `w` is drawn at depth `NEAR / w`, so a
/// nearer face has a greater depth, and nothing is drawn nearer than `NEAR`, one Q3 step. The
/// near plane may cut a box whose nearest point lies within `is_eyed`'s reach of the eye, so
/// such a box is drawn over the whole screen instead, at the nearest depth, and a lack's box
/// nearer still (pxl-render raster.wgsl). Every pixel then takes it, and a pixel whose ray
/// does not enter it walks, so it never falls through to what lies behind.
pub const NEAR: f32 = 1.0 / (1 << FRAC_W) as f32;

/// The factor a stale voxel's depth is multiplied by: 2^-64. Every stale fragment then lies
/// behind every fresh one, since a fresh fragment as far as `REACH_MOST` still has a depth of
/// `NEAR / REACH_MOST`, which is greater. The stale fragments keep their order among
/// themselves, because an f32's relative precision is the same at every exponent its normal
/// numbers reach.
pub const STALE_DEPTH: f32 = 1.0 / (1u64 << 32) as f32 / (1u64 << 32) as f32;

/// The farthest an eye's ray may reach with every fresh box still before every stale one
/// (`STALE_DEPTH`): 2^55 units, 2^31 times around the world.
pub const REACH_MOST: f32 = (1u64 << 55) as f32;
const _: () = assert!(
    NEAR / REACH_MOST > STALE_DEPTH,
    "a fresh box at the farthest reach lies before every stale one"
);

/// Whether the box `lo`..`hi`, in units from the eye, lies so near the eye that the raster's
/// near plane (`NEAR`) may cut it, given the tangents `tan` of the screen's half-angles: where
/// its nearest point lies within twice `NEAR` at the screen's corner. A lack's box that holds
/// the eye counts too.
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

/// Whether the eye's `right`, `up` and `forward` turn the other way from the axes that
/// `corner` winds front faces for, that is, whether right × up points along forward.
pub fn is_flipped(right: [f32; 3], up: [f32; 3], forward: [f32; 3]) -> bool {
    let x = [
        right[1] * up[2] - right[2] * up[1],
        right[2] * up[0] - right[0] * up[2],
        right[0] * up[1] - right[1] * up[0],
    ];
    x[0] * forward[0] + x[1] * forward[1] + x[2] * forward[2] > 0.0
}

/// Corner `k` of a cube's 36, which build.rs writes into pxl-render raster.wgsl: two
/// triangles a face, the faces in order across X, then Y, then Z, the low side first. Each
/// triangle is wound counter-clockwise seen from outside, so a face turned to the eye is wound
/// counter-clockwise on the screen (`facing_face_ccw`) and the raster culls the faces turned
/// away. Where `flipped`, each triangle is wound the other way.
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

    /// Every face of a box that the eye stands outside of and before, projected as the raster
    /// projects it (pxl-render raster.wgsl `vs`), is wound counter-clockwise on the screen where
    /// its outward normal is turned to the eye, and clockwise where it is turned away, so a front
    /// face is kept.
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
