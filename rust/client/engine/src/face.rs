// pxl-engine/face.rs

//! wgsl-port: the word that says what lies around a face of a cell, which the cast hands
//! to the screen pass in a ray's own words (`ray::FACE_WORD`). Across each of the face's four
//! edges lies the air, the next cell of a flat run, or a cell rising in front of the face. At
//! each of its four corners there may be a cell rising there alone. The word is asked of the
//! world's one query, about the cells beside the cell at the cell's own zoom (pxl-render
//! face.wgsl). It is asked once for each face a hit enters, and handed with the hit to every pixel
//! that shows the cell, so every pixel of a cell is painted from the same bits in a frame,
//! whatever else the screen shows around it.
//!
//! A face's edges are named by the direction one crosses each in, along the face: `+u`, `−u`,
//! `+v`, `−v`, where `u` and `v` are the two axes after the face's own, in turn (for x: y, z;
//! for y: z, x; for z: x, y). Its corners are named by the two edges that meet there: corner
//! `k` lies across edge `k & 1` and edge `2 + (k >> 1)`, giving `+u+v`, `−u+v`, `+u−v`,
//! `−u−v`.

use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::Zoom;

/// The number of a face's edges, which is also the number of its corners.
pub const EDGE_CNT: u32 = 4;
/// The word's three fields, in the order they are asked; what one field holds decides what
/// the next asks. Bit `k` of a field belongs to edge `k`, or to corner `k`. A rise bit is set
/// where a cell hides what is in front of the face across the edge: the face meets a wall
/// there and closes in toward it.
pub const RISE_SHIFT: u32 = 0;
/// A line bit is set where the edge meets the air: neither the cell in front of the face
/// across it nor the cell beside it hides it. Such an edge is convex or a step down, and it is
/// drawn as a line.
pub const LINE_SHIFT: u32 = EDGE_CNT;
/// A corner bit is set where a cell hides what is in front of the face across the corner and
/// none does across either edge that meets there, since such an edge closes the corner in
/// already.
pub const CORNER_SHIFT: u32 = 2 * EDGE_CNT;
pub const BITS: u32 = 3 * EDGE_CNT;

/// The bits of a face of a cell filled short of its cube (`answer::fill_eighths`): a box
/// inset in the cube, apart from every cell around it, so every edge meets the air and nothing
/// rises across one. As a neighbor, such a cell hides no face (`answer::whole`).
pub const BOX: u32 = ((1 << EDGE_CNT) - 1) << LINE_SHIFT;

/// The bits of face `face` of a cell, numbered as a ray numbers faces: 2·axis, plus one where
/// the outward normal points down the axis. `hides(d)` says whether the cell `d` cells away
/// hides its cube whole (`answer::whole`). Bit `i` answers the `i`th cell asked, and a cell
/// is not asked where a bit already set decides it. The asking lives in this one place,
/// because a shader inlines every call.
pub fn word(face: u32, mut hides: impl FnMut([i32; 3]) -> bool) -> u32 {
    let axis = face / 2;
    let n = unit(axis, 1 - 2 * (face & 1) as i32);
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut w = 0;
    let mut i = 0;
    while i < BITS {
        let (field, k) = (i / EDGE_CNT, i % EDGE_CNT);
        let (a, b) = (k & 1, 2 + (k >> 1));
        // The cell asked, and whether what is set already leaves it open.
        let (d, open) = match field {
            0 => (add(edge(u, v, k), n), true),
            1 => (edge(u, v, k), w >> (RISE_SHIFT + k) & 1 == 0),
            _ => (
                add(add(edge(u, v, a), edge(u, v, b)), n),
                w >> RISE_SHIFT & (1 << a | 1 << b) == 0,
            ),
        };
        // A line bit where the cell beside does not hide; a rise or corner bit where the cell in
        // front does.
        if open && hides(d) != (field == 1) {
            w |= 1 << i;
        }
        i += 1;
    }
    w
}

/// The node `d` away from node `c` at `zoom`, with X wrapped around the world (`wrap_idx`).
pub fn beside(zoom: Zoom, c: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [
        wrap_idx(zoom, c[0].wrapping_add(d[0])),
        c[1] + d[1],
        c[2] + d[2],
    ]
}

/// The step across edge `k`, along `u` or `v`.
fn edge(u: u32, v: u32, k: u32) -> [i32; 3] {
    unit(if k < 2 { u } else { v }, 1 - 2 * (k & 1) as i32)
}

// [naga] Each axis is set by itself rather than indexed, as in cast.rs `grid_of`.
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

    /// The bits of `face` among the cells `filled`, given as offsets from the cell, and how many
    /// cells were asked.
    fn bits(face: u32, filled: &[[i32; 3]]) -> (u32, u32) {
        let mut asked = 0;
        let w = word(face, |d| {
            asked += 1;
            filled.contains(&d)
        });
        (w, asked)
    }

    /// A flat ground: every cell at `z` 0 in a 5 × 5 square around the cell, and the cells
    /// `more`.
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
    fn flat_run_unlined() {
        let (w, asked) = bits(TOP, &ground(&[]));
        assert_eq!(w, 0);
        assert_eq!(asked, 12);
    }

    #[test]
    fn lone_cell_lined() {
        let (w, _) = bits(TOP, &[]);
        assert_eq!(w, 0b1111 << LINE_SHIFT);
        let (w, _) = bits(WEST, &[]);
        assert_eq!(w, 0b1111 << LINE_SHIFT);
    }

    #[test]
    fn wall_closes_once() {
        // A wall along +x, one cell up, beside every y of the cell, at the top face's edge +u
        // (u = x): that edge has no line and rises, and the corners at it are closed by the edge,
        // not again by the diagonal.
        let (w, asked) = bits(TOP, &ground(&[[1, -1, 1], [1, 0, 1], [1, 1, 1]]));
        assert_eq!(w, 1 << RISE_SHIFT);
        // The wall alone decides the edge, and the edges decide its corners, so three cells go
        // unasked.
        assert_eq!(asked, 12 - 1 - 2);
    }

    #[test]
    fn corner_cell_closes() {
        // One cell up at −x, +y: corner −u+v, which is corner 1.
        let (w, _) = bits(TOP, &ground(&[[-1, 1, 1]]));
        assert_eq!(w, 1 << (CORNER_SHIFT + 1));
    }

    #[test]
    fn step_down_lined() {
        // The ground drops one past +y, so the top's edge +v (v = y) meets the air.
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
    fn side_face_axes() {
        // The west face (−x, u = y, v = z) of a cell on flat ground: the ground running on in front
        // of it closes the face's foot in, across −v, and the air above and to either side lines it.
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
        // A cell in front of it, one west and one up (−x, then +z), makes a rise across +v.
        let (w, _) = bits(WEST, &[[-1, 0, 1]]);
        assert_eq!(w, 1 << (RISE_SHIFT + 2) | 0b1011 << LINE_SHIFT);
    }

    #[test]
    fn x_wraps_at_edge() {
        let z = 3;
        let last = (1 << (z - 1)) - 1;
        assert_eq!(beside(z, [last, 0, 0], [1, 0, 0]), [-(1 << (z - 1)), 0, 0]);
        assert_eq!(beside(z, [0, 5, 7], [-1, 1, -1]), [-1, 6, 6]);
    }
}
