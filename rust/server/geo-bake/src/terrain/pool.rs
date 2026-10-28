// pxl-geo-bake/terrain/pool.rs

//! Whether a set's ways' trees fit the client's pool wherever the eye is
//! (pxl-spec-core `way::tree::POOL_FAMILIES`): a fact of the set, not of a
//! tile, as a window is a fixed grid of bricks about the eye laid at any
//! brick, across tiles. Each brick's tree is built as a GPU builds it
//! (pxl-spec `terrain::fill_fit`), once, as its tile is made, and what it
//! takes kept beside the tile (`line`, pyramid.rs `FITS`); each window's
//! most is the most any placement of it holds, a sliding sum over its
//! zoom's bricks; the windows together need `terrain::pool_needs`.

use std::collections::BTreeMap;

use fnv::FnvHashMap;
use pxl_spec::bake::tile::terrain::{self, Fit, TerrainTile};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::Tile;
use pxl_spec_core::way::tree::POOL_FAMILIES;
use pxl_spec_core::window::{ACROSS, BRICK};

use crate::pyramid::Staged;

/// The bricks a tile is a side.
const SIDE: u32 = TILE_CELLS / BRICK;

/// A zoom's bricks, each the families its tree holds, by its index at its
/// cells; and the most a brick takes before its fold.
#[derive(Default)]
struct Held {
    bricks: BTreeMap<Zoom, FnvHashMap<[i32; 2], u64>>,
    taken: u64,
    /// Each tile's that takes anything, as the set will keep it.
    kept: Vec<(Tile, String)>,
}

/// The set's ways' trees fit the pool, and what it will keep of each tile
/// that takes anything; where they do not, the window that holds the most
/// at the zoom that needs it, its first brick and its tiles named.
pub fn fits(set: &Staged<'_>, workers: usize) -> Result<Vec<(Tile, String)>, String> {
    let tiles: Vec<Tile> = set.tiles()?.into_values().flatten().collect();
    let base = set.base.fits()?;
    let base = base.as_ref();
    let chunk = tiles.len().div_ceil(workers.max(1)).max(1);
    let parts: Vec<Result<Held, String>> = std::thread::scope(|s| {
        let runs: Vec<_> = tiles
            .chunks(chunk)
            .map(|part| s.spawn(move || held(set, base, part)))
            .collect();
        runs.into_iter()
            .map(|r| r.join().unwrap_or_else(|_| Err("a worker panicked".into())))
            .collect()
    });
    let mut all = Held::default();
    for part in parts {
        let part = part?;
        all.taken = all.taken.max(part.taken);
        for (zoom, bricks) in part.bricks {
            all.bricks.entry(zoom).or_default().extend(bricks);
        }
        all.kept.extend(part.kept);
    }
    let worst: BTreeMap<Zoom, ([i32; 2], u64)> = all
        .bricks
        .iter()
        .map(|(&cell, bricks)| (cell, most(bricks)))
        .collect();
    let needs = terrain::pool_needs(|cell| worst.get(&cell).map_or(0, |w| w.1), all.taken);
    if needs <= u64::from(POOL_FAMILIES) {
        all.kept.sort_unstable_by_key(|(p, _)| *p);
        return Ok(all.kept);
    }
    let (cell, (first, n)) = worst
        .iter()
        .max_by_key(|(_, w)| w.1)
        .map(|(&c, &w)| (c, w))
        .unwrap_or((0, ([0; 2], 0)));
    let zoom = GROUND.tile_zoom(cell) as Zoom;
    let mut under: Vec<String> = (0..ACROSS as i32 * ACROSS as i32)
        .map(|k| {
            Tile::new(
                zoom,
                [
                    (first[0] + k % ACROSS as i32).div_euclid(SIDE as i32),
                    (first[1] + k / ACROSS as i32).div_euclid(SIDE as i32),
                ],
            )
            .name()
        })
        .collect();
    under.sort_unstable();
    under.dedup();
    Err(format!(
        "its ways' trees need {needs} of the pool's {POOL_FAMILIES} families: the window at cells {cell} from brick ({}, {}) holds {n}, over tiles {}, and a brick takes {} before its fold",
        first[0],
        first[1],
        under.join(" "),
        all.taken
    ))
}

/// The bricks of `tiles` the set holds, each its tree's families: as each
/// tile kept them, or made again from the tile where the set as it is
/// keeps nothing (`base` none, a set baked before it did).
fn held(
    set: &Staged<'_>,
    base: Option<&FnvHashMap<Tile, String>>,
    tiles: &[Tile],
) -> Result<Held, String> {
    let mut out = Held::default();
    for &p in tiles {
        let fit = match set.kept(p, base)? {
            Some(None) => continue,
            Some(Some(line)) => of_line(&line).map_err(|e| format!("{}: {e}", p.name()))?,
            None => {
                let Some(tile) = set.read::<TerrainTile>(p)? else {
                    continue;
                };
                terrain::fill_fit(p, &tile, |_, _| {}).map_err(|(at, why)| {
                    format!("{}: {}", p.name(), crate::terrain::unfit(at, why))
                })?
            }
        };
        if fit == Fit::default() {
            continue;
        }
        let cell = GROUND.cell_zoom(p.zoom);
        let bricks = out.bricks.entry(cell).or_default();
        for &(k, held) in &fit.held {
            let at = [
                p.at[0] * SIDE as i32 + (k % SIDE) as i32,
                p.at[1] * SIDE as i32 + (k / SIDE) as i32,
            ];
            bricks.insert(at, u64::from(held));
        }
        out.taken = out.taken.max(u64::from(fit.taken));
        out.kept.push((p, line(&fit)));
    }
    Ok(out)
}

/// What a tile takes as the set keeps it beside the tile: the most a brick
/// takes, then each brick with ways as its index and the families it holds.
pub fn line(fit: &Fit) -> String {
    let mut out = fit.taken.to_string();
    for (k, held) in &fit.held {
        out += &format!(" {k}:{held}");
    }
    out
}

fn of_line(line: &str) -> Result<Fit, String> {
    let bad = || format!("kept as {line:?}, not as `line` writes");
    let mut words = line.split_whitespace();
    let taken = words.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let held = words
        .map(|w| {
            let (k, n) = w.split_once(':').ok_or_else(bad)?;
            Ok((k.parse().map_err(|_| bad())?, n.parse().map_err(|_| bad())?))
        })
        .collect::<Result<_, String>>()?;
    Ok(Fit { held, taken })
}

/// The placement of `ACROSS` bricks a side holding the most of `bricks`:
/// its first brick, and how many.
fn most(bricks: &FnvHashMap<[i32; 2], u64>) -> ([i32; 2], u64) {
    let across = ACROSS as i32;
    // Each brick summed into the columns of `ACROSS` over it, then those
    // into the rows of `ACROSS` beside them: each placement's sum by its first.
    let mut columns: FnvHashMap<[i32; 2], u64> = FnvHashMap::default();
    for (&[x, y], &n) in bricks {
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
    placed
        .into_iter()
        .max_by_key(|&(at, n)| (n, std::cmp::Reverse(at)))
        .unwrap_or(([0; 2], 0))
}
