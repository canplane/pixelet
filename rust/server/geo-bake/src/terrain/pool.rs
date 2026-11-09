// pxl-geo-bake/terrain/pool.rs

//! Whether a set's ways' trees fit the client's pool wherever the eye is
//! (pxl-engine `way::tree::POOL_CHILD_GROUPS`): a fact of the set, not of a
//! tile, as a window is a fixed grid of bricks about the eye laid at any
//! brick, across tiles. Each brick's tree is built as a client builds it
//! (pxl-spec `terrain::fill_fit`), once, as its tile is made, and what it
//! takes is the tile's fit (`line`, pyramid.rs `FITS`); each window's
//! most is the most any placement of it holds, a sliding sum over its
//! zoom's bricks; the windows together need `terrain::pool_needs`.

use std::collections::BTreeMap;

use fnv::FnvHashMap;
use pxl_engine::clipmap::{ACROSS, BRICK};
use pxl_engine::way::tree::POOL_CHILD_GROUPS;
use pxl_spec::bake::tile::terrain::{self, Fit};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::pyramid::Staged;

/// The bricks a tile is a side.
const SIDE: u32 = TILE_CELLS / BRICK;

/// A zoom's bricks, each the child groups its tree holds, by its index at its
/// cells; and the most a brick takes before its fold.
#[derive(Default)]
struct Tally {
    bricks: BTreeMap<Zoom, FnvHashMap<[i32; 2], u64>>,
    peak_group_cnt: u64,
    /// The fit of each tile that takes anything.
    tiles: Vec<(Quadrant, String)>,
}

/// The set's ways' trees fit the pool, and the fit of each tile that takes
/// anything; where they do not, the window that holds the most
/// at the zoom that needs it, its first brick and its tiles named.
pub(crate) fn check(
    set: &Staged<'_>,
    worker_cnt: usize,
) -> Result<Vec<(Quadrant, String)>, String> {
    let tiles: Vec<Quadrant> = set.tiles()?.into_values().flatten().collect();
    let base = match set.base.fits()? {
        Some(fits) => fits,
        None if set.base.by_zoom()?.is_empty() => FnvHashMap::default(),
        None => return Err("the set holds tiles and no fits: bake it again".into()),
    };
    let base = &base;
    let chunk = tiles.len().div_ceil(worker_cnt.max(1)).max(1);
    let parts: Vec<Result<Tally, String>> = std::thread::scope(|s| {
        let runs: Vec<_> = tiles
            .chunks(chunk)
            .map(|part| s.spawn(move || set_bricks(set, base, part)))
            .collect();
        runs.into_iter()
            .map(|r| r.join().unwrap_or_else(|_| Err("a worker panicked".into())))
            .collect()
    });
    let mut all = Tally::default();
    for part in parts {
        let part = part?;
        all.peak_group_cnt = all.peak_group_cnt.max(part.peak_group_cnt);
        for (zoom, bricks) in part.bricks {
            all.bricks.entry(zoom).or_default().extend(bricks);
        }
        all.tiles.extend(part.tiles);
    }
    let worst: BTreeMap<Zoom, ([i32; 2], u64)> = all
        .bricks
        .iter()
        .map(|(&cell_zoom, bricks)| (cell_zoom, most(bricks)))
        .collect();
    let needs = terrain::pool_needs(|cell_zoom| worst.get(&cell_zoom).map_or(0, |w| w.1));
    if needs <= u64::from(POOL_CHILD_GROUPS) {
        all.tiles.sort_unstable_by_key(|(p, _)| *p);
        return Ok(all.tiles);
    }
    let (cell_zoom, (first, n)) = worst
        .iter()
        .max_by_key(|(_, w)| w.1)
        .map(|(&c, &w)| (c, w))
        .unwrap_or((0, ([0; 2], 0)));
    let zoom = GROUND.tile_zoom(cell_zoom) as Zoom;
    let mut under: Vec<String> = (0..ACROSS as i32 * ACROSS as i32)
        .map(|k| {
            Quadrant::new(
                zoom,
                [
                    (first[0] + k % ACROSS as i32).div_euclid(SIDE as i32),
                    (first[1] + k / ACROSS as i32).div_euclid(SIDE as i32),
                ],
            )
            .key()
        })
        .collect();
    under.sort_unstable();
    under.dedup();
    Err(format!(
        "its ways' trees need {needs} of the pool's {POOL_CHILD_GROUPS} child groups: the window at cells {cell_zoom} from brick ({}, {}) holds {n}, over tiles {}, and a brick takes {} before its fold",
        first[0],
        first[1],
        under.join(" "),
        all.peak_group_cnt
    ))
}

/// The bricks of `tiles` the set holds, each its tree's child groups, off each
/// tile's fit.
fn set_bricks(
    set: &Staged<'_>,
    base: &FnvHashMap<Quadrant, String>,
    tiles: &[Quadrant],
) -> Result<Tally, String> {
    let mut out = Tally::default();
    for &p in tiles {
        let Some(text) = set.fit_of(p, base)? else {
            continue;
        };
        let fit = of_line(&text).map_err(|e| format!("{}: {e}", p.key()))?;
        if fit == Fit::default() {
            continue;
        }
        let cell_zoom = GROUND.cell_zoom(p.zoom);
        let bricks = out.bricks.entry(cell_zoom).or_default();
        for &(k, groups) in &fit.bricks {
            let at = [
                p.at[0] * SIDE as i32 + (k % SIDE) as i32,
                p.at[1] * SIDE as i32 + (k / SIDE) as i32,
            ];
            bricks.insert(at, u64::from(groups));
        }
        out.peak_group_cnt = out.peak_group_cnt.max(u64::from(fit.peak_group_cnt));
        out.tiles.push((p, line(&fit)));
    }
    Ok(out)
}

/// A tile's fit as the set holds it: the most a brick
/// takes, then each brick with ways as its index and the child groups it holds.
pub fn line(fit: &Fit) -> String {
    let mut out = fit.peak_group_cnt.to_string();
    for (k, groups) in &fit.bricks {
        out += &format!(" {k}:{groups}");
    }
    out
}

fn of_line(line: &str) -> Result<Fit, String> {
    let bad = || format!("a fit of {line:?}, not as `line` writes");
    let mut words = line.split_whitespace();
    let taken = words.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let groups = words
        .map(|w| {
            let (k, n) = w.split_once(':').ok_or_else(bad)?;
            Ok((k.parse().map_err(|_| bad())?, n.parse().map_err(|_| bad())?))
        })
        .collect::<Result<_, String>>()?;
    Ok(Fit {
        bricks: groups,
        peak_group_cnt: taken,
    })
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

#[cfg(test)]
#[path = "pool.test.rs"]
mod test;
