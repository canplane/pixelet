// pxl-spec-core/raster.rs

//! gpu-portable: the samples' stops drawn as boxes, so the rasterizer picks
//! each drawn pixel's block (pxl-shader raster.rs `sample_box_cs`, cast.rs
//! `cast_cs`; pxl-render gpu/ray/raster.rs, raster.wgsl). Each sample that
//! stopped is a box, the block as the cast stops on it (`cast::drawn`),
//! unless the sample before it across or up stopped alike, which some
//! sample before it in turn draws. A box draws its face turned to the eye
//! across the axis its sample entered by, a lack's every face turned to
//! it, and a face keeps a pixel only where the pixel's own ray enters the
//! box across that face's axis (`cast::entry`'s test; raster.wgsl `fs`): the
//! face a sample's ray reached through air is one the pixel's may show only
//! where it enters across the same. A ray enters a box through one face, so
//! what is kept lies where the ray enters, at the depth the raster draws
//! it, and the depth test keeps the nearest box the pixel's ray enters of
//! every sample's, leaving its sample's number (`id_of`): the pixel's block
//! where a hit, and where a lack, a node it walks itself.

/// The draw's indirect args, a buffer of their own (`ARGS_WORDS`):
/// `ARGS_VERTICES`, `ARGS_INSTANCES`, then two words the draw reads as 0;
/// then the dispatch over the boxes that asks what lies about each one's
/// face (pxl-shader `face_cs`), `FACES_LANES` boxes a workgroup
/// (`ARGS_FACES`), its workgroups raised as boxes are written, the other
/// two 1. One dispatch's 65535 workgroups hold 4.2M boxes, a box a sample
/// at most and a sample 16 pixels: past 67 Mpx.
/// Then the boxes' buffer, `BOX_WORDS` a box: its planes in units from the
/// eye's origin, lowest then highest, f32 bits, its sample's number
/// (`id_of`), and the axis its sample entered across, with `EYED` where the
/// box lies about the eye (`eyed`).
pub const ARGS_VERTICES: u32 = 0;
pub const ARGS_INSTANCES: u32 = 1;
pub const ARGS_FACES: u32 = 4;
pub const ARGS_WORDS: u32 = 7;
pub const FACES_LANES: u32 = 64;
pub const BOX_LO: u32 = 0;
pub const BOX_HI: u32 = 3;
pub const BOX_ID: u32 = 6;
pub const BOX_AXIS: u32 = 7;
pub const BOX_WORDS: u32 = 8;
pub const EYED: u32 = 1 << 8;

/// The words the boxes' buffer takes for `samples` samples, each a box at
/// most.
pub const fn words(samples: u32) -> u32 {
    samples * BOX_WORDS
}

/// A box's corners drawn: its three faces turned to the eye, one an axis,
/// two triangles each (`corner`), a face not drawn left a point.
pub const VERTICES: u32 = 18;

/// A sample's number as the raster leaves it at a pixel: its index among
/// the samples (`beam::Grid::sample_at`'s, `j · samples[0] + i`) plus one,
/// so 0 is none, and `LACK` where the sample lacks.
pub const LACK: u32 = 1 << 31;

pub const fn id_of(sample: u32, lacks: bool) -> u32 {
    (sample + 1) | if lacks { LACK } else { 0 }
}

/// The sample an id names (`id_of`), none for 0.
pub const fn sample_of(id: u32) -> u32 {
    (id & !LACK) - 1
}

/// The raster's reversed depth: a face at view depth `w` drawn at `NEAR /
/// w`, nearer greater, nothing nearer than `NEAR`, a Q3 step. A box whose
/// nearest point lies within `eyed`'s reach of the eye may be cut by it,
/// and is drawn over the whole screen instead, at the nearest depth, a
/// lack's nearer still (raster.wgsl): every pixel then takes it, and one
/// whose ray does not enter it walks, never falling to what lies behind.
pub const NEAR: f32 = 1.0 / (1 << crate::FRAC_W) as f32;

/// Drawn pixels a face is grown by in its own plane, at each corner's
/// distance: so a pixel's middle on its edge is not lost to the
/// rasterizer's rounding. The face stays in its plane, so a pixel's depth
/// is still where its ray meets the face, and what the growth alone covers
/// fails the fragment's exact test (raster.wgsl `fs`): it costs fragments
/// and changes no pick.
pub const GROWN: f32 = 0.25;

/// Whether the box `lo`..`hi`, in units from the eye, lies so near it that
/// the raster's near plane may cut it (`NEAR`), `tan` the tangents of the
/// screen's half-angles: its nearest point within `NEAR` at the screen's
/// corner, twice over. A lack's box the eye is in is that too.
pub fn eyed(lo: [f32; 3], hi: [f32; 3], tan: [f32; 2]) -> bool {
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
pub fn flipped(right: [f32; 3], up: [f32; 3], forward: [f32; 3]) -> bool {
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
/// screen (`a_face_turned_to_the_eye_is_wound_counter_clockwise`) and the
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
    fn a_face_turned_to_the_eye_is_wound_counter_clockwise() {
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
            let flip = flipped(r, u, f);
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

    /// An id names its sample again, and says whether it lacks.
    #[test]
    fn an_id_names_its_sample() {
        for (s, lacks) in [(0, false), (7, true), ((1 << 25) - 1, false)] {
            let id = id_of(s, lacks);
            assert_ne!(id, 0);
            assert_eq!(sample_of(id), s);
            assert_eq!(id & LACK != 0, lacks);
        }
    }
}
