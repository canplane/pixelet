// pxl-render/mesh/chunk.rs

//! A chunk's blocks (`pxl-spec block/sample.rs` `Blocks`) to one mesh: each
//! block a ray stopped on a box, whole, shaded by the blocks about it; alike boxes merged across the cells,
//! then written out, each phase resumable under a frame budget. The mesh is
//! written through caller blocks of any size (`Step::NeedMesh`,
//! `Step::Full`).

use pxl_spec::block::sample::{Blocks, Refused};
use pxl_spec::block::Block;
use pxl_spec::space::camera::grid::CHUNK_CELLS;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::math::scalar::round;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::look::color::{linear, srgb};
use crate::mesh::paint::ground::{ground, look};
use crate::mesh::paint::palette::layer_mean;
use crate::mesh::word::taps_of;
use pxl_spec::block::mask::{filled, mask as portable_mask, ALL_CREASES, ALL_FACES};

/// The fine format: corner and width in u16 steps of its cell, floor and height
/// in u16 mesh steps, sRGB color, four layer taps, face mask.
///
/// The coarse format drops the taps (only cells a leaf's or finer are
/// textured) and holds corner and width in a byte an axis, in eighth cells:
/// a chunk draws `CHUNK_CELLS` cells across. 24 and 16 bytes a box, against
/// 84 a vertex once.
pub const FINE_BOX_WORDS: u32 = 6;
pub const COARSE_BOX_WORDS: u32 = 4;

pub const fn box_words(fine: bool) -> u32 {
    if fine {
        FINE_BOX_WORDS
    } else {
        COARSE_BOX_WORDS
    }
}

/// A cell is 256 steps, so a chunk's widest row fits a u16; heights are held
/// against each mesh's own floor and step instead, since a block's z is the
/// world's.
const XY_STEPS: f64 = 256.0;
const _: () = assert!(CHUNK_CELLS as f64 * XY_STEPS <= 65536.0, "a fine box's corner and width live in a u16 each");
const COARSE_XY_STEPS: f64 = 8.0;
const _: () = assert!(CHUNK_CELLS * (COARSE_XY_STEPS as u32) <= 256, "a coarse box's corner and width live in a byte each");
const Z_STEPS: f64 = 65535.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heights {
    pub floor: f64,
    pub step: f64,
}

impl Default for Heights {
    fn default() -> Self {
        Heights { floor: 0.0, step: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mesh {
    pub box_cnt: u32,
    pub heights: Heights,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Working,
    /// Returned by every step until `provide_mesh` hands a block of at least one box.
    NeedMesh(u32),
    /// The block is full: the caller takes its boxes and provides the next.
    /// Returned by every step until then.
    Full,
    Done,
    /// The heap refused the workspace: the bake is over, and asked for again later.
    Refused,
}

/// A raw range so the context owes it no lifetime; the caller keeps it valid
/// until the block is full, or the bake finishes or aborts.
#[derive(Clone, Copy)]
struct MeshTarget {
    ptr: *mut u32,
    words: usize,
    first: u32,
}

/// A box: `size` cells from the cell `cell`, its blocks `z[0]..z[1]` up
/// from the floor, one look, and the faces it shows.
#[derive(Clone, Copy)]
struct Piece {
    cell: [u32; 2],
    size: [u32; 2],
    z: [u32; 2],
    rgb: [u8; 3],
    taps: u32,
    mask: u32,
}

impl Piece {
    /// What two boxes side by side must share to be one.
    fn key(&self) -> ([u32; 2], [u8; 3], u32, u32) {
        (self.z, self.rgb, self.taps, self.mask)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Faces,
    Merge,
    Emit,
}

pub struct BakeContext {
    pieces: Vec<Piece>,
    /// Every word is written before read, so the target is never cleared.
    mesh: Option<MeshTarget>,
    heights: Heights,
    box_words: u32,
    /// A coarse box carries one color, the tile mean when the pack is drawn or the
    /// swatch when not, so it matches the leaves under it either way.
    textured: bool,
    /// Boxes a step makes or writes, the asker's to set.
    boxes: u32,
    /// A block's side and the chunk's first block, in its blocks.
    unit: f64,
    first: [i64; 3],
    phase: Phase,
    /// The seen blocks met so far.
    met: usize,
    emit_at: usize,
    mesh_words: u32,
    box_cnt: u32,
    open: bool,
    done: bool,
    refused: bool,
}

impl Default for BakeContext {
    fn default() -> BakeContext {
        BakeContext {
            pieces: Vec::new(),
            mesh: None,
            heights: Heights::default(),
            box_words: FINE_BOX_WORDS,
            textured: true,
            boxes: 1,
            unit: 1.0,
            first: [0; 3],
            phase: Phase::Faces,
            met: 0,
            emit_at: 0,
            mesh_words: 0,
            box_cnt: 0,
            open: false,
            done: false,
            refused: false,
        }
    }
}

impl BakeContext {
    /// A chunk of cells at `cell`, its blocks composed into what each step
    /// is handed (`pxl-kernel query/block.rs` `World::compose`). Cells a
    /// leaf's or finer are textured; `boxes` boxes a step are made or
    /// written. One bake at a time.
    pub fn begin(&mut self, cell: Zoom, textured: bool, boxes: u32) {
        xpute_kit::ensure!(!self.open, ENOTRECOVERABLE);
        (self.boxes, self.textured) = (boxes.max(1), textured);
        self.unit = units(cell);
        self.box_words = box_words(cell >= GROUND.cell());
        self.pieces.clear();
        (self.phase, self.met, self.emit_at) = (Phase::Faces, 0, 0);
        (self.mesh, self.mesh_words, self.box_cnt) = (None, 0, 0);
        (self.open, self.done, self.refused) = (true, false, false);
    }

    pub fn step(&mut self, blocks: &Blocks) -> Step {
        if self.done {
            return Step::Done;
        }
        if self.mesh_words > 0 && self.mesh.is_none() {
            return Step::NeedMesh(self.mesh_words);
        }
        if self.full() {
            return Step::Full;
        }
        xpute_kit::ensure!(self.open, ENOTRECOVERABLE);
        match self.phase {
            Phase::Faces => {
                let grown = self.faces(blocks);
                self.refused |= grown.is_err();
            }
            Phase::Merge => self.merge(),
            Phase::Emit => self.done = self.emit(),
        }
        if self.refused {
            self.abort();
            return Step::Refused;
        }
        if self.done {
            Step::Done
        } else if self.mesh_words > 0 && self.mesh.is_none() {
            Step::NeedMesh(self.mesh_words)
        } else if self.full() {
            Step::Full
        } else {
            Step::Working
        }
    }

    fn full(&self) -> bool {
        self.mesh.is_some_and(|t| (self.box_cnt - t.first) as usize * self.box_words as usize >= t.words)
    }

    pub fn left_words(&self) -> u32 {
        self.mesh_words - self.box_cnt * self.box_words
    }

    pub fn heights(&self) -> Heights {
        self.heights
    }

    /// At least one box, and never more than what is left.
    ///
    /// # Safety
    /// `ptr` must address `words` words, 4-aligned, and stay valid and
    /// otherwise untouched until the block is full, or `finish` or `abort`.
    pub unsafe fn provide_mesh(&mut self, ptr: *mut u32, words: u32) {
        xpute_kit::ensure!(self.mesh_words > 0 && (self.mesh.is_none() || self.full()), ENOTRECOVERABLE);
        let left = self.left_words();
        xpute_kit::ensure!(words >= self.box_words.min(left) && words <= left, ENOTRECOVERABLE, words, left);
        let words = words - words % self.box_words;
        self.mesh = Some(MeshTarget {
            ptr,
            words: words as usize,
            first: self.box_cnt,
        });
    }

    pub fn finish(&mut self) -> Mesh {
        xpute_kit::ensure!(self.done && self.open, ENOTRECOVERABLE);
        self.open = false;
        self.mesh = None;
        Mesh {
            box_cnt: self.box_cnt,
            heights: self.heights,
        }
    }

    pub fn abort(&mut self) {
        self.open = false;
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.pieces.clear();
    }

    /// The seen blocks' boxes, `boxes` of them a step at most: each a box,
    /// whole.
    fn faces(&mut self, blocks: &Blocks) -> Result<(), Refused> {
        self.first = blocks.first();
        let end = blocks.seen.len().min(self.met + self.boxes as usize);
        while self.met < end {
            let b = blocks.seen[self.met];
            self.met += 1;
            // Asked of the world as the preview shows it, or after it moved
            // since the cast, the block a ray stopped on may not fill:
            // nothing to draw.
            let Some(block) = blocks.block(b).flatten().filter(|b| b.fills()) else { continue };
            let [x, y, z] = [0, 1, 2].map(|a| (b[a] - self.first[a]) as u32);
            let (rgb, taps) = self.drawn(block);
            grow(&mut self.pieces)?;
            self.pieces.push(Piece {
                cell: [x, y],
                size: [1, 1],
                z: [z, z + 1],
                rgb,
                taps,
                mask: mask(blocks, b),
            });
        }
        if self.met == blocks.seen.len() {
            self.phase = Phase::Merge;
        }
        Ok(())
    }

    /// What `block` is drawn as: fine, its swatch and taps; coarse, one
    /// color, a summary's ground `mean`.
    fn drawn(&self, block: Block) -> ([u8; 3], u32) {
        let (rgb, layer) = look(block);
        if self.box_words == FINE_BOX_WORDS {
            return (rgb, taps_of(layer));
        }
        let rgb = match block {
            // A summary's ground is one color: the mean of the looks it folds,
            // its face's alone where a way or the wall recolors it.
            Block::Ground(face, fold) if fold.contains(&face) => {
                let sum = fold.iter().fold([0.0; 3], |acc, &g| {
                    let (rgb, layer) = ground(g);
                    let c = self.layer_color(rgb, layer).map(linear);
                    [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]]
                });
                sum.map(|v| srgb(v / fold.len() as f64))
            }
            _ => self.layer_color(rgb, layer),
        };
        (rgb, 0)
    }

    /// A coarse box's color of a swatch and layer: the layer's mean where the
    /// pack is drawn, so it matches the leaves under it.
    fn layer_color(&self, rgb: [u8; 3], layer: u8) -> [u8; 3] {
        if self.textured {
            layer_mean(u32::from(layer), rgb)
        } else {
            rgb
        }
    }

    /// Alike boxes side by side merged along a row, then those rows' boxes
    /// down the rows: two alike side by side show no face to each other,
    /// since each fills across the other's.
    fn merge(&mut self) {
        let p = &mut self.pieces;
        p.sort_unstable_by_key(|q| (q.key(), q.cell[1], q.cell[0]));
        p.dedup_by(|b, a| {
            let next = a.key() == b.key() && a.cell[1] == b.cell[1] && a.cell[0] + a.size[0] == b.cell[0];
            if next {
                a.size[0] += b.size[0];
            }
            next
        });
        p.sort_unstable_by_key(|q| (q.key(), q.cell[0], q.size[0], q.cell[1]));
        p.dedup_by(|b, a| {
            let next = a.key() == b.key() && (a.cell[0], a.size[0]) == (b.cell[0], b.size[0]) && a.cell[1] + a.size[1] == b.cell[1];
            if next {
                a.size[1] += b.size[1];
            }
            next
        });
        let [lo, hi] = p.iter().fold([u32::MAX, 0], |[lo, hi], q| [lo.min(q.z[0]), hi.max(q.z[1])]);
        // One block for the whole mesh: one a lane moved hundreds of MB in a frame.
        self.mesh_words = p.len() as u32 * self.box_words;
        self.heights = match lo < hi {
            true => {
                let floor = self.z_of(lo).floor();
                Heights {
                    floor,
                    step: ((self.z_of(hi) - floor) / Z_STEPS).max(f64::MIN_POSITIVE),
                }
            }
            false => Heights::default(),
        };
        self.phase = Phase::Emit;
    }

    /// The height of the block `k` up from the floor, in units.
    fn z_of(&self, k: u32) -> f64 {
        (self.first[2] + i64::from(k)) as f64 * self.unit
    }

    fn emit(&mut self) -> bool {
        let end = self.pieces.len().min(self.emit_at + self.boxes as usize);
        while self.emit_at < end {
            if self.full() {
                return false;
            }
            let p = self.pieces[self.emit_at];
            let min = [f64::from(p.cell[0]) * self.unit, f64::from(p.cell[1]) * self.unit, self.z_of(p.z[0])];
            let size = [f64::from(p.size[0]) * self.unit, f64::from(p.size[1]) * self.unit, self.z_of(p.z[1]) - self.z_of(p.z[0])];
            let target = self.mesh.or_bug(Errno::ENOTRECOVERABLE);
            // SAFETY: provide_mesh's contract; the range is valid and untouched until the bake closes.
            let m = unsafe { core::slice::from_raw_parts_mut(target.ptr, target.words) };
            let words = self.box_words as usize;
            let at = (self.box_cnt - target.first) as usize * words;
            write_box(&mut m[at..at + words], min, size, p.rgb, p.taps, p.mask, self.unit, self.heights);
            self.box_cnt += 1;
            self.emit_at += 1;
        }
        self.emit_at >= self.pieces.len()
    }
}

/// Room for one more, asked for first.
fn grow(v: &mut Vec<Piece>) -> Result<(), Refused> {
    pxl_spec::block::sample::grow(v, 1)
}

/// What the box of the block `b` shows and what the shader shades it by,
/// read off the blocks about it `shades` names (pxl-portable `mask`).
fn mask(blocks: &Blocks, b: [i64; 3]) -> u32 {
    portable_mask(filled(|[x, y, z]| {
        blocks
            .block([b[0] + i64::from(x), b[1] + i64::from(y), b[2] + i64::from(z)])
            .or_bug(Errno::ENOTRECOVERABLE)
            .is_some_and(Block::fills)
    }))
}

/// Every lateral edge a crease, so each shades as a silhouette.
const LONE_BOX_FACES: u32 = ALL_FACES | ALL_CREASES;

/// Where a mesh holding a box cornered at `min` stands, and the box in it: on
/// the whole unit under it, as a chunk's mesh stands, so a texture's repeat is
/// read off the mesh's own coordinates.
pub fn on_whole_units(min: V3) -> (V3, V3) {
    let origin = min.map(f64::floor);
    (origin, [min[0] - origin[0], min[1] - origin[1], min[2] - origin[2]])
}

/// The hover ghost's box at `offset` in its mesh (`on_whole_units`), written
/// as a bake writes one so it is lit and tiled as a placed cube is.
pub fn write_lone_box(m: &mut [u32], offset: V3, size: f64, rgb: [u8; 3], layer: u8) -> Heights {
    xpute_kit::ensure!(m.len() == FINE_BOX_WORDS as usize, ENOTRECOVERABLE);
    let heights = Heights {
        floor: 0.0,
        step: (offset[2] + size) / Z_STEPS,
    };
    write_box(m, offset, [size; 3], rgb, taps_of(layer), LONE_BOX_FACES, size, heights);
    heights
}

#[allow(clippy::too_many_arguments)]
fn write_box(m: &mut [u32], min: V3, size: [f64; 3], rgb: [u8; 3], taps: u32, faces: u32, unit: f64, heights: Heights) {
    // Both ends rounded, not the size: boxes stacked meet at one step, never
    // a step over or apart.
    let (z0, z1) = (steps(min[2] - heights.floor, heights.step), steps(min[2] + size[2] - heights.floor, heights.step));
    let z = z0 | ((z1 - z0) << 16);
    if m.len() == COARSE_BOX_WORDS as usize {
        let xy = unit / COARSE_XY_STEPS;
        // The byte holds `width − 1`: zero width would be no box, and a whole mesh across fits.
        let wide = |v: f64| steps(v, xy).clamp(1, 256) - 1;
        m[0] = faces;
        m[1] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
        m[2] = z;
        m[3] = steps(min[0], xy).min(255) | (steps(min[1], xy).min(255) << 8) | (wide(size[0]) << 16) | (wide(size[1]) << 24);
        return;
    }
    let xy = unit / XY_STEPS;
    m[0] = steps(min[0], xy) | (steps(min[1], xy) << 16);
    m[1] = steps(size[0], xy) | (steps(size[1], xy) << 16);
    m[2] = z;
    m[3] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
    m[4] = taps;
    m[5] = faces;
}

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}

pub fn box_bounds(m: &[u32], unit: f64, heights: Heights) -> (V3, [f64; 3]) {
    let (lo, span) = box_z(m);
    let z0 = heights.floor + lo as f64 * heights.step;
    let dz = span as f64 * heights.step;
    if m.len() == COARSE_BOX_WORDS as usize {
        let xy = unit / COARSE_XY_STEPS;
        let byte = |k: u32| ((m[3] >> k) & 0xff) as f64;
        return ([byte(0) * xy, byte(8) * xy, z0], [(byte(16) + 1.0) * xy, (byte(24) + 1.0) * xy, dz]);
    }
    let xy = unit / XY_STEPS;
    let half = |w: u32, high: bool| (if high { w >> 16 } else { w & 0xffff }) as f64 * xy;
    ([half(m[0], false), half(m[0], true), z0], [half(m[1], false), half(m[1], true), dz])
}

fn box_z(m: &[u32]) -> (u32, u32) {
    (m[2] & 0xffff, m[2] >> 16)
}
