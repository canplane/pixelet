// pxl-geo-bake/terrain/pool.rs

//! Whether a set's ways' trees fit the client's pool wherever the eye is
//! (pxl-portable `ways::tree::POOL_BLOCKS`): a fact of the set, not of a
//! tile, as a window is a fixed grid of squares about the eye laid at any
//! square, across tiles. Each square's tree is built as a GPU builds it
//! (pxl-spec `terrain::square_blocks`); each window's most is the most any
//! placement of it holds, a sliding sum over its zoom's squares; the
//! windows together need `terrain::pool_needs`.

use std::collections::BTreeMap;

use fnv::FnvHashMap;
use pxl_portable::ways::tree::{POOL_BLOCKS, SQUARE_BLOCKS};
use pxl_portable::window::{ACROSS, HANDED_WORDS, SQUARE, SQUARE_WORDS, WAYS_SQUARE_SEGMENTS};
use pxl_spec::bake::tile::terrain::{self, TerrainTile};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::Tile;

use crate::pyramid::Staged;

/// The squares a tile is a side.
const SIDE: u32 = TILE_CELLS / SQUARE;

/// A zoom's squares, each the blocks its tree holds, by its index at its
/// cells; and the most a square takes before its fold.
#[derive(Default)]
struct Held {
    squares: BTreeMap<Zoom, FnvHashMap<[i32; 2], u64>>,
    taken: u64,
}

/// The set's ways' trees fit the pool; where they do not, the window that
/// holds the most at the zoom that needs it, its first square and its
/// tiles named.
pub fn fits(set: &Staged<'_>, workers: usize) -> Result<(), String> {
    let tiles: Vec<Tile> = set.tiles()?.into_values().flatten().collect();
    let chunk = tiles.len().div_ceil(workers.max(1)).max(1);
    let parts: Vec<Result<Held, String>> = std::thread::scope(|s| {
        let runs: Vec<_> = tiles.chunks(chunk).map(|part| s.spawn(move || held(set, part))).collect();
        runs.into_iter().map(|r| r.join().unwrap_or_else(|_| Err("a worker panicked".into()))).collect()
    });
    let mut all = Held::default();
    for part in parts {
        let part = part?;
        all.taken = all.taken.max(part.taken);
        for (zoom, squares) in part.squares {
            all.squares.entry(zoom).or_default().extend(squares);
        }
    }
    let worst: BTreeMap<Zoom, ([i32; 2], u64)> = all.squares.iter().map(|(&cell, squares)| (cell, most(squares))).collect();
    let needs = terrain::pool_needs(|cell| worst.get(&cell).map_or(0, |w| w.1), all.taken);
    if needs <= u64::from(POOL_BLOCKS) {
        return Ok(());
    }
    let (cell, (first, n)) = worst.iter().max_by_key(|(_, w)| w.1).map(|(&c, &w)| (c, w)).unwrap_or((0, ([0; 2], 0)));
    let zoom = GROUND.tile_zoom(cell) as Zoom;
    let mut under: Vec<String> = (0..ACROSS as i32 * ACROSS as i32)
        .map(|k| Tile::new(zoom, [(first[0] + k % ACROSS as i32).div_euclid(SIDE as i32), (first[1] + k / ACROSS as i32).div_euclid(SIDE as i32)]).name())
        .collect();
    under.sort_unstable();
    under.dedup();
    Err(format!(
        "its ways' trees need {needs} of the pool's {POOL_BLOCKS} blocks: the window at cells {cell} from square ({}, {}) holds {n}, over tiles {}, and a square takes {} before its fold",
        first[0],
        first[1],
        under.join(" "),
        all.taken
    ))
}

/// The squares of `tiles` the set holds, each its tree's blocks.
fn held(set: &Staged<'_>, tiles: &[Tile]) -> Result<Held, String> {
    let mut scratch = vec![(0, 0); WAYS_SQUARE_SEGMENTS as usize];
    let (mut handed, mut lane) = (vec![0; HANDED_WORDS as usize], vec![0; SQUARE_WORDS as usize]);
    // A square's room: the most a build takes, past the block none holds.
    let mut pool = vec![0; 16 * (SQUARE_BLOCKS as usize + 1)];
    let mut out = Held::default();
    for &p in tiles {
        let Some(tile) = set.read::<TerrainTile>(p)? else { continue };
        let blocks = terrain::square_blocks(p, &tile, &mut scratch, [&mut handed, &mut lane, &mut pool]);
        let cell = GROUND.cell_zoom(p.zoom);
        for (k, [taken, held]) in blocks.into_iter().enumerate() {
            if taken == u32::MAX {
                return Err(format!("{}: its square {k} takes more than SQUARE_BLOCKS ({SQUARE_BLOCKS}) of its ways' tree's blocks", p.name()));
            }
            let at = [p.at[0] * SIDE as i32 + (k as u32 % SIDE) as i32, p.at[1] * SIDE as i32 + (k as u32 / SIDE) as i32];
            out.squares.entry(cell).or_default().insert(at, u64::from(held));
            out.taken = out.taken.max(u64::from(taken));
        }
    }
    Ok(out)
}

/// The placement of `ACROSS` squares a side holding the most of `squares`:
/// its first square, and how many.
fn most(squares: &FnvHashMap<[i32; 2], u64>) -> ([i32; 2], u64) {
    let across = ACROSS as i32;
    // Each square summed into the columns of `ACROSS` over it, then those
    // into the rows of `ACROSS` beside them: each placement's sum by its first.
    let mut columns: FnvHashMap<[i32; 2], u64> = FnvHashMap::default();
    for (&[x, y], &n) in squares {
        for d in 0..across {
            *columns.entry([x, y - d]).or_default() += n;
        }
    }
    let mut placed: FnvHashMap<[i32; 2], u64> = FnvHashMap::default();
    for (&[x, y], &n) in &columns {
        for d in 0..across {
            *placed.entry([x - d, y]).or_default() += n;
        }
    }
    placed.into_iter().max_by_key(|&(at, n)| (n, std::cmp::Reverse(at))).unwrap_or(([0; 2], 0))
}
