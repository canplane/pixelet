// pxl-kernel/layer/model/occupancy.rs

//! What a model fills, as a grid in its own frame: the volume a placed mesh
//! is taken to occupy where something asks what stands where (world/access.rs
//! `support`), rather than its triangles, which stay what is drawn and hit.
//!
//! A cell is filled where a triangle touches it, or where it is enclosed:
//! the cells reached from outside the grid without crossing a touched one are
//! the outside, and every other cell is the model's. So a closed mesh — a
//! rock, a car's body — is solid through, and an open one — a tree's leaf
//! cards — is only its shell. This is the conservative voxelization binvox
//! and Minecraft's schematic importers make, kept as runs up the model's
//! height, since what reads it asks a column at a time.
//!
//! Made once, when the model is sealed, from its vertices alone: the mesh's
//! own fact, the same however it is registered, placed or sized. Which tier
//! an instance rounds it to is the instance's (layer/decor/volume.rs).

/// How many cells the grid lays along the model's longest side: how finely
/// the mesh's shape is kept, and no physical size. A tier rounds outward on
/// top of it, so what it costs is a model whose instances are much larger
/// than a tier — a building sixty-four cells long rounds by up to a cell
/// more — against a grid of 64³ at most to fill once when it loads.
const CELLS_LONGEST: f32 = 64.0;

/// A model's filled cells, in its own frame (glTF's: Y up), unscaled.
pub struct Occupancy {
    /// The grid's corner and its cell's side.
    pub lo: [f32; 3],
    pub cell: f32,
    /// The mesh's longest side.
    pub longest: f32,
    /// The filled cells as runs up Y: x, z, then y from and y to, exclusive.
    pub runs: Vec<[u16; 4]>,
}

impl Occupancy {
    /// The grid of the mesh `positions` (x, y, z a vertex) and `indices` (a
    /// triangle three) make, `CELLS_LONGEST` cells along its longest side.
    /// None for a mesh with no extent.
    pub fn of(positions: &[f32], indices: &[u32]) -> Option<Occupancy> {
        let points = positions.as_chunks::<3>().0;
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        if lo[0] > hi[0] {
            return None;
        }
        let longest = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f32::max);
        let cell = longest / CELLS_LONGEST;
        if !cell.is_finite() || cell <= 0.0 {
            return None;
        }
        // One cell of outside around the grid, for the flood to start from
        // and go around through.
        let dims: [usize; 3] = core::array::from_fn(|k| ((hi[k] - lo[k]) / cell).floor() as usize + 1 + 2);
        let index = |[x, y, z]: [usize; 3]| (z * dims[1] + y) * dims[0] + x;
        let mut grid = vec![EMPTY; dims[0] * dims[1] * dims[2]];
        let at = |p: &[f32; 3]| -> [f32; 3] { core::array::from_fn(|k| (p[k] - lo[k]) / cell + 1.0) };
        for tri in indices.as_chunks::<3>().0 {
            let Some(t) = tri.iter().map(|&i| points.get(i as usize).map(at)).collect::<Option<Vec<_>>>() else {
                continue;
            };
            let t = [t[0], t[1], t[2]];
            let from: [usize; 3] = core::array::from_fn(|k| t.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min).floor().max(1.0) as usize);
            let to: [usize; 3] = core::array::from_fn(|k| (t.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max).floor() as usize).min(dims[k] - 2));
            for z in from[2]..=to[2] {
                for y in from[1]..=to[1] {
                    for x in from[0]..=to[0] {
                        let c = [x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5];
                        if touches(c, &t) {
                            grid[index([x, y, z])] = SURFACE;
                        }
                    }
                }
            }
        }
        let mut stack = vec![[0usize; 3]];
        grid[0] = OUTSIDE;
        while let Some(c) = stack.pop() {
            for (k, d) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
                let v = c[k] as isize + d;
                if v < 0 || v as usize >= dims[k] {
                    continue;
                }
                let mut n = c;
                n[k] = v as usize;
                let i = index(n);
                if grid[i] == EMPTY {
                    grid[i] = OUTSIDE;
                    stack.push(n);
                }
            }
        }
        let mut runs = Vec::new();
        for z in 1..dims[2] - 1 {
            for x in 1..dims[0] - 1 {
                let mut from = None;
                for y in 1..dims[1] {
                    let filled = y < dims[1] - 1 && grid[index([x, y, z])] != OUTSIDE;
                    match (filled, from) {
                        (true, None) => from = Some(y),
                        (false, Some(f)) => {
                            runs.push([x - 1, z - 1, f - 1, y - 1].map(|v| v as u16));
                            from = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        Some(Occupancy { lo, cell, longest, runs })
    }
}

const EMPTY: u8 = 0;
const SURFACE: u8 = 1;
const OUTSIDE: u8 = 2;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Whether triangle `t` touches the unit cell centered at `c`: no
/// separating axis among the cell's three, the triangle's normal and the
/// nine crossings of their edges (Akenine-Möller, "Fast 3D Triangle-Box
/// Overlap Testing", 2001). Touching counts, so a face lying on a cell's
/// boundary fills the cells on both sides of it, as rounding outward wants.
fn touches(c: [f32; 3], t: &[[f32; 3]; 3]) -> bool {
    const H: f32 = 0.5;
    let v = t.map(|p| sub(p, c));
    let e = [sub(v[1], v[0]), sub(v[2], v[1]), sub(v[0], v[2])];
    let apart = |axis: [f32; 3]| {
        let p = v.map(|x| dot(x, axis));
        let r = H * (axis[0].abs() + axis[1].abs() + axis[2].abs());
        p.iter().copied().fold(f32::INFINITY, f32::min) > r || p.iter().copied().fold(f32::NEG_INFINITY, f32::max) < -r
    };
    let units = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if units.iter().any(|&u| apart(u)) || apart(cross(e[0], e[1])) {
        return false;
    }
    !units.iter().any(|&u| e.iter().any(|&ei| apart(cross(u, ei))))
}

#[cfg(test)]
#[path = "occupancy.test.rs"]
mod test;
