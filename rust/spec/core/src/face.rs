// pxl-spec-core/face.rs

//! gpu-portable: what lies about a face of a block, as the cast hands it to
//! the screen pass in a ray's own words (`ray::FACES`): across each of the
//! face's four edges, the air, the next block of a flat run, or a block
//! rising in front of the face; and at each of its four corners, a block
//! rising there alone. Asked of the world's one query, at the block's own
//! zoom, of the blocks beside it (pxl-shader `faced`), once where a
//! sample stops and handed with its stop to every pixel that shows that
//! block: so every pixel of a block is painted by the same bits in a frame,
//! whatever the screen shows about it.
//!
//! The face's edges are named by the way across each along the face, `+u`,
//! `−u`, `+v`, `−v`, `u` and `v` the axes after the face's own in turn
//! (x: y, z; y: z, x; z: x, y); its corners by the two edges that meet
//! there, corner `k` across edge `k & 1` and edge `2 + (k >> 1)`: `+u+v`,
//! `−u+v`, `+u−v`, `−u−v`.

use crate::{wrap_index, Zoom};

/// A face's edges, and its corners.
pub const EDGES: u32 = 4;
/// Bit `k` of each field is edge `k`'s or corner `k`'s, the fields in the
/// order they are asked, each deciding what the next asks. A block hides
/// what is in front of the face across the edge: the face meets a wall there, and
/// closes in towards it.
pub const RISE_SHIFT: u32 = 0;
/// The edge meets the air: neither the block in front of the face across
/// it nor the one beside it hides it, a convex edge or a step down, drawn as
/// a line.
pub const LINE_SHIFT: u32 = EDGES;
/// A block hides what is in front of the face across the corner, and none across
/// either edge meeting there, which closes the corner in already.
pub const CORNER_SHIFT: u32 = 2 * EDGES;
pub const BITS: u32 = 3 * EDGES;

/// The bits of a face of a block kept short of its cube
/// (`answer::kept`), a box inset in it apart from every block about it:
/// every edge meets the air, and nothing rises across one. Beside it, it is
/// no block that hides for a face's bits (`answer::whole`).
pub const BOX: u32 = ((1 << EDGES) - 1) << LINE_SHIFT;

/// The bits of face `face` (a ray's: 2·axis, plus one where its outward
/// normal points down the axis) of a block, `hides(d)` whether the block
/// `d` blocks from it hides its cube whole (`answer::whole`): bit `i` answers the
/// `i`th block asked, and a block is not asked where a bit already set
/// decides it. One place asks, as a shader inlines each.
pub fn word(face: u32, mut hides: impl FnMut([i32; 3]) -> bool) -> u32 {
    let axis = face / 2;
    let n = unit(axis, 1 - 2 * (face & 1) as i32);
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut w = 0;
    let mut i = 0;
    while i < BITS {
        let (field, k) = (i / EDGES, i % EDGES);
        let (a, b) = (k & 1, 2 + (k >> 1));
        // The block asked, and whether what is set already leaves it open.
        let (d, open) = match field {
            0 => (add(edge(u, v, k), n), true),
            1 => (edge(u, v, k), w >> (RISE_SHIFT + k) & 1 == 0),
            _ => (
                add(add(edge(u, v, a), edge(u, v, b)), n),
                w >> RISE_SHIFT & (1 << a | 1 << b) == 0,
            ),
        };
        // A line where the block beside does not hide; a rise or a corner
        // where the one in front does.
        if open && hides(d) != (field == 1) {
            w |= 1 << i;
        }
        i += 1;
    }
    w
}

/// The node `d` from node `c` at `zoom`, X named as the world names it
/// going round (`wrap_index`).
pub fn beside(zoom: Zoom, c: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [
        wrap_index(zoom, c[0].wrapping_add(d[0])),
        c[1] + d[1],
        c[2] + d[2],
    ]
}

/// Edge `k`'s way across, along `u` or `v`.
fn edge(u: u32, v: u32, k: u32) -> [i32; 3] {
    unit(if k < 2 { u } else { v }, 1 - 2 * (k & 1) as i32)
}

// [naga] each axis by itself, not an index (cast.rs `grid_of`).
fn unit(axis: u32, s: i32) -> [i32; 3] {
    [
        if axis == 0 { s } else { 0 },
        if axis == 1 { s } else { 0 },
        if axis == 2 { s } else { 0 },
    ]
}

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: u32 = 4;
    const WEST: u32 = 1;

    /// The bits of `face` among the blocks `filled`, offsets from the
    /// block's, and how many blocks were asked.
    fn bits(face: u32, filled: &[[i32; 3]]) -> (u32, u32) {
        let mut asked = 0;
        let w = word(face, |d| {
            asked += 1;
            filled.contains(&d)
        });
        (w, asked)
    }

    /// Every block of the layer below `z` 1 of a 5 × 5 about the block,
    /// and those `more`.
    fn ground(more: &[[i32; 3]]) -> [[i32; 3]; 64] {
        let mut out = [[9, 9, 9]; 64];
        let mut i = 0;
        for x in -2..=2 {
            for y in -2..=2 {
                out[i] = [x, y, 0];
                i += 1;
            }
        }
        for m in more {
            out[i] = *m;
            i += 1;
        }
        out
    }

    #[test]
    fn a_flat_run_has_no_line_and_no_corner() {
        let (w, asked) = bits(TOP, &ground(&[]));
        assert_eq!(w, 0);
        assert_eq!(asked, 12);
    }

    #[test]
    fn a_lone_block_is_lined_all_round() {
        let (w, _) = bits(TOP, &[]);
        assert_eq!(w, 0b1111 << LINE_SHIFT);
        let (w, _) = bits(WEST, &[]);
        assert_eq!(w, 0b1111 << LINE_SHIFT);
    }

    #[test]
    fn a_wall_closes_in_its_edge_and_its_corners_once() {
        // A wall along +x, its blocks one up beside the block's every y,
        // the edge +u of a top (u = x): no line there, the edge rising, and
        // the corners at it closed by the edge, not again by the diagonal.
        let (w, asked) = bits(TOP, &ground(&[[1, -1, 1], [1, 0, 1], [1, 1, 1]]));
        assert_eq!(w, 1 << RISE_SHIFT);
        // Its edge decided by the wall alone; its corners by the edges.
        assert_eq!(asked, 12 - 1 - 2);
    }

    #[test]
    fn a_block_at_a_corner_alone_closes_that_corner() {
        // One block up at −x, +y: corner −u+v, 1.
        let (w, _) = bits(TOP, &ground(&[[-1, 1, 1]]));
        assert_eq!(w, 1 << (CORNER_SHIFT + 1));
    }

    #[test]
    fn a_step_down_is_lined() {
        // The ground dropping one past +y: the edge +v (v = y) of the top
        // meets the air.
        let mut low = ground(&[]);
        for b in low.iter_mut() {
            if b[1] > 0 && b[2] == 0 {
                b[2] = -1;
            }
        }
        let (w, _) = bits(TOP, &low);
        assert_eq!(w, 1 << (LINE_SHIFT + 2));
    }

    #[test]
    fn a_side_face_reads_its_own_axes() {
        // The west face (−x, u = y, v = z) of a block on flat ground: the
        // ground running on in front of it closes its foot in, across −v;
        // above and to either side the air, lined.
        let (w, _) = bits(
            WEST,
            &[
                [0, 0, -1],
                [-1, 0, -1],
                [0, 1, -1],
                [0, -1, -1],
                [-1, 1, -1],
                [-1, -1, -1],
            ],
        );
        assert_eq!(w, 1 << (RISE_SHIFT + 3) | 0b0111 << LINE_SHIFT);
        // A block in front of it, one west and one up: −x then +z, a rise
        // across +v.
        let (w, _) = bits(WEST, &[[-1, 0, 1]]);
        assert_eq!(w, 1 << (RISE_SHIFT + 2) | 0b1011 << LINE_SHIFT);
    }

    #[test]
    fn x_goes_round_beside_the_world_s_edge() {
        let z = 3;
        let last = (1 << (z - 1)) - 1;
        assert_eq!(beside(z, [last, 0, 0], [1, 0, 0]), [-(1 << (z - 1)), 0, 0]);
        assert_eq!(beside(z, [0, 5, 7], [-1, 1, -1]), [-1, 6, 6]);
    }
}
