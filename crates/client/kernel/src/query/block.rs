// pxl-kernel/query/block.rs

//! What is in each block, as one composition (`pxl-spec block/stand.rs`
//! `compose`) of the ground, its bridges, tunnels and piers, and what is
//! placed: a chunk composes its column with it and draws
//! that, and the world's query composes the column of the chunk drawing the
//! cell it is asked of at the zoom what is placed there is held at, so what
//! it answers is what that chunk draws. Neither reads a layer's storage.

use std::collections::HashMap;

use pxl_spec::bake::constant::{EntryAt, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::sample::{beside_index, span, Around, ChunkSample, Refused, APRON, COL_SPAN};
use pxl_spec::block::stand::{compose, stands_of, z_unit, Block, Stand};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{base_of, count, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, offset_in, Node};
use pxl_spec::space::layer::lattice::{GROUND, GROUND_FOLD};
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::ray::{hit, Hit, Ray, Struck};
use super::world::World;
use crate::layer::object::store::{Cells, Store};
use crate::layer::terrain::Source;
use crate::layer::Answer;
use crate::layer::Query;

/// Zooms runs at `runs` are finer than the cells of the tile drawn in grid
/// cells at `cell`: none but past the leaves.
fn finer(cell: Zoom, runs: Zoom) -> u32 {
    u32::from(runs - GROUND.cell_zoom(GROUND.read_zoom(cell)))
}

/// The zoom a chunk column of grid cells at `cell` is composed at: the
/// records its cells fold, one fold finer, where anything is held finer than
/// its cells, the ground included; else its cells, which then fold nothing.
/// From its own column alone: its apron is composed at that zoom, as a
/// neighbour of another zoom is.
pub fn runs(objects: &Store, column: Node<2>, cell: Zoom) -> Zoom {
    let finest = objects.bounds(column, cell).value().flatten().map_or(GROUND.cell(), |b| b.finest.max(GROUND.cell()));
    if cell < finest {
        cell + GROUND_FOLD
    } else {
        cell
    }
}

/// A chunk column's records, which its blocks are composed from: the ground
/// its tile cells are read from and the tiles beside it, what is placed
/// folded to the zoom its runs are composed at, and what of that a preview
/// hides or cuts to air.
#[derive(Clone, Copy)]
pub struct Column<'a> {
    pub column: Node<2>,
    cell: Zoom,
    pub runs: Zoom,
    pub ground: Source<'a>,
    pub beside: [Option<Source<'a>>; 8],
    objects: Cells<'a>,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
}

/// What composing a column works in, kept by whoever composes so a settled
/// composition takes nothing from the heap: the leaves' sample a column past
/// them is refined from, and what is placed as folded.
pub struct Composer {
    leaf: ChunkSample,
    folded: HashMap<[i64; 3], u32>,
}

impl Default for Composer {
    fn default() -> Composer {
        Composer {
            leaf: ChunkSample::of_span(COL_SPAN),
            folded: HashMap::new(),
        }
    }
}

/// A column's blocks: its ground, and what stands on it.
pub struct Blocks {
    pub sample: ChunkSample,
    pub stands: Vec<(u32, Stand)>,
}

impl Default for Blocks {
    fn default() -> Blocks {
        Blocks {
            sample: ChunkSample::of_span(COL_SPAN),
            stands: Vec::new(),
        }
    }
}

impl<'a> World<'a> {
    /// The records of the chunk column `node` of grid cells at `cell`.
    pub(super) fn column(&self, (zoom, [x, y, _]): Node<3>, cell: Zoom) -> Answer<Column<'a>> {
        let column = (zoom, [x, y]);
        let runs = runs(self.objects, column, cell);
        let (read, read_cell) = read_of(column, cell, runs);
        let mut beside = [None; 8];
        for ([qx, qy], p) in GROUND.read_beside(read, read_cell, APRON) {
            // A tile is read at its own cells.
            beside[beside_index(qx, qy)] = self.terrain.read((p.zoom, [ivec::X(p.tile), ivec::Y(p.tile), 0]), GROUND.grid_zoom(p.zoom)).value();
        }
        let (rz, [rx, ry]) = read;
        // What is placed is all resident: its answer is always exact.
        let objects = self.objects.read((zoom, [x, y, 0]), runs).value().or_bug(Errno::ENOTRECOVERABLE);
        self.terrain.read((rz, [rx, ry, 0]), read_cell).map(|ground| Column {
            column,
            cell,
            runs,
            ground,
            beside,
            objects,
            hide: self.hide,
            carve: self.carve,
        })
    }
}

/// The column whose tile cells are read and its grid cells' zoom: past the
/// leaves, the one over it whose runs are its tile's cells.
fn read_of((zoom, at): Node<2>, cell: Zoom, runs: Zoom) -> (Node<2>, Zoom) {
    let k = finer(cell, runs);
    ((zoom - k as Zoom, ancestor_of(at, k)), cell - k as Zoom)
}

impl Column<'_> {
    /// Its blocks, into `into`.
    pub fn compose(&self, c: &mut Composer, into: &mut Blocks) -> Result<(), Refused> {
        self.sample(&mut c.leaf, &mut into.sample)?;
        self.stand(&mut c.folded, into)
    }

    /// Its ground, into `sample`; past the leaves refined from the leaves'
    /// sample, `leaf`.
    fn sample(&self, leaf: &mut ChunkSample, sample: &mut ChunkSample) -> Result<(), Refused> {
        let around = Around {
            own: self.ground.view(),
            beside: self.beside.map(|b| b.map(|s| s.view())),
        };
        let (read, cell) = read_of(self.column, self.cell, self.runs);
        let (x0, y0) = GROUND.read_at(read, cell);
        let first = [i64::from(x0), i64::from(y0)];
        let span = span(self.column.0, self.runs);
        match finer(self.cell, self.runs) {
            0 => sample.read(&around, first, span),
            k => {
                let at = offset_in(self.column.1, k).map(|v| v as u32 * (span - 2 * APRON));
                leaf.read(&around, first, GROUND.read_side(read.0, cell) + 2 * APRON)?;
                sample.refine(leaf, k, at, span)
            }
        }
    }

    /// What is placed, folded to its runs as a preview shows it, standing on
    /// `into`'s ground.
    fn stand(&self, folded: &mut HashMap<[i64; 3], u32>, into: &mut Blocks) -> Result<(), Refused> {
        let apron = i64::from(APRON);
        self.objects.fold(apron, self.hide, folded)?;
        if let Some(carve) = self.carve {
            self.objects.carve(apron, carve, folded)?;
        }
        let (from, _) = self.objects.column_cells(apron);
        stands_of(&mut into.sample, folded, (from[0] + apron, from[1] + apron), self.runs, &mut into.stands)
    }
}

/// A run `[lo, hi)` in units of one block, and whether it fills it, as air does not.
#[derive(Clone, Copy, Debug)]
pub(super) struct Run {
    pub(super) lo: f64,
    pub(super) hi: f64,
    pub(super) block: Block,
    pub(super) fills: bool,
}

/// The ground's top in a sub-column, in units, and the block it is drawn as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub block: GroundId,
}

/// A ground cell's blocks: its sub-columns, `across` a side at the runs'
/// zoom, each its runs lowest first and its surface, where it has ground.
pub(super) struct Cell<'s> {
    /// Its corner, in the asker's frame.
    corner: [f64; 2],
    side: f64,
    across: usize,
    /// A run's cell, upright.
    unit: f64,
    at: &'s [usize],
    runs: &'s [Run],
    surface: &'s [Option<Surface>],
}

impl Cell<'_> {
    /// Every sub-column's runs, one after another.
    pub(super) fn all(&self) -> &[Run] {
        self.runs
    }

    /// The highest ground top among its sub-columns; none where none has ground.
    pub(super) fn surface_top(&self) -> Option<f64> {
        self.surface.iter().flatten().map(|s| s.top).reduce(f64::max)
    }

    /// The runs and surface of the sub-column over `[x, y]`.
    pub(super) fn under(&self, [x, y]: [f64; 2]) -> (&[Run], Option<Surface>) {
        let index = |v: f64, c: f64| (((v - c) / self.side).floor().max(0.0) as usize).min(self.across - 1);
        let k = index(y, self.corner[1]) * self.across + index(x, self.corner[0]);
        (&self.runs[self.at[k]..self.at[k + 1]], self.surface[k])
    }

    /// The first ground `ray` enters in the cell within `t1`; what it starts
    /// inside is not met. What is placed is the objects' to strike.
    pub(super) fn ground_hit(&self, ray: &Ray, t1: f64) -> Option<Hit> {
        let mut best: Option<(f64, usize, Aabb)> = None;
        for k in 0..self.across * self.across {
            let min = [self.corner[0] + (k % self.across) as f64 * self.side, self.corner[1] + (k / self.across) as f64 * self.side];
            for r in self.runs[self.at[k]..self.at[k + 1]].iter().filter(|r| r.fills && matches!(r.block, Block::Ground(_))) {
                let b = Aabb {
                    min: [min[0], min[1], r.lo],
                    max: [min[0] + self.side, min[1] + self.side, r.hi],
                };
                let Some((enter, _, axis)) = b.span_along(ray.origin, ray.direction) else { continue };
                if enter > 0.0 && enter <= t1 && best.is_none_or(|(t, _, _)| enter < t) {
                    best = Some((enter, axis, b));
                }
            }
        }
        let (t, axis, b) = best?;
        let mut normal: V3 = [0.0; 3];
        normal[axis] = if ray.direction[axis] > 0.0 { -1.0 } else { 1.0 };
        // The one block of the run it enters.
        let cells = ((b.max[2] - b.min[2]) / self.unit).round() as i64;
        let cell = match axis {
            2 if normal[2] > 0.0 => cells - 1,
            2 => 0,
            _ => (((ray.at(t)[2] - b.min[2]) / self.unit).floor() as i64).clamp(0, cells - 1),
        };
        let lo = b.min[2] + cell as f64 * self.unit;
        let cube = Aabb {
            min: [b.min[0], b.min[1], lo],
            max: [b.max[0], b.max[1], lo + self.unit],
        };
        Some(hit(ray, t, normal, cube, Struck::Ground))
    }
}

/// The query's composer and blocks and what it reads off one cell, kept
/// between asks so a settled ask takes nothing from the heap.
struct Scratch {
    composer: Composer,
    blocks: Blocks,
    at: Vec<usize>,
    runs: Vec<Run>,
    surface: Vec<Option<Surface>>,
}

static SCRATCH: Global<Scratch> = Global::new();

impl<'a> World<'a> {
    /// The ground cell `cell` of the tile zoom `zoom`, X in the asker's frame,
    /// composed as the chunks drawing it as held compose it, unpreviewed. A
    /// leaf cell's runs are at the zoom what is placed in it is held at; past
    /// the leaves' tiles a cell is the chunks' at it, what is placed folded
    /// into it as they fold it. `f` asks the world nothing: it reads the
    /// query's one scratch.
    pub(super) fn cell<R>(&self, cell: [i64; 2], zoom: Zoom, f: impl FnOnce(&Cell) -> R) -> Answer<R> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let at = GROUND.cell_of(zoom, &base_of(cell_zoom, [cell[0], cell[1], 0]));
        let [x, y] = at.node().1;
        // Asked at the finest, so a bound one past its cells caps nothing.
        let held = self.objects.bounds((cell_zoom, [x, y]), OBJECT_FINEST).value().flatten().map(|b| b.finest);
        let runs = match held {
            Some(finest) if zoom == LEAF_ZOOM => finest.max(cell_zoom),
            _ => cell_zoom,
        };
        // As a chunk composing it at `runs` does, of grid cells one fold coarser.
        let world = World { hide: None, carve: None, ..*self };
        let answer = world.column((cell_zoom, [x, y, 0]), runs - GROUND_FOLD);
        let Some(column) = answer.value() else { return Answer::Nothing(at.tile()) };
        let s = SCRATCH.get(|| Scratch {
            composer: Composer::default(),
            blocks: Blocks::default(),
            at: Vec::new(),
            runs: Vec::new(),
            surface: Vec::new(),
        });
        if column.compose(&mut s.composer, &mut s.blocks).is_err() {
            return Answer::Nothing(at.tile());
        }
        let (sample, stands) = (&s.blocks.sample, &s.blocks.stands);
        let unit = z_unit(runs);
        let across = count(runs, cell_zoom) as usize;
        let (row, o) = (sample.span as usize, APRON as usize);
        s.at.clear();
        s.runs.clear();
        s.surface.clear();
        for sy in 0..across {
            for sx in 0..across {
                let i = (o + sy) * row + o + sx;
                let first = stands.partition_point(|s| (s.0 as usize) < i);
                let next = stands.partition_point(|s| (s.0 as usize) <= i);
                s.at.push(s.runs.len());
                s.surface.push(sample.has_ground(i).then(|| Surface {
                    top: f64::from(sample.ele[i]) * unit,
                    block: sample.block[i],
                }));
                let runs = &mut s.runs;
                let _ = compose(sample, &stands[first..next], i, i32::MIN, |lo, hi, block| {
                    runs.push(Run {
                        lo: f64::from(lo) * unit,
                        hi: f64::from(hi + 1) * unit,
                        block,
                        fills: block.fills(),
                    });
                    Ok::<_, ()>(())
                });
            }
        }
        s.at.push(s.runs.len());
        let c = Cell {
            corner: cell.map(|v| v as f64 * units(cell_zoom)),
            side: units(runs),
            across,
            unit,
            at: &s.at,
            runs: &s.runs,
            surface: &s.surface,
        };
        answer.map(|_| f(&c))
    }
}
