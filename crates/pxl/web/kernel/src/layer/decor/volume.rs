// pxl-kernel/layer/decor/volume.rs

//! What decor fills, for what asks what stands where (world/access.rs
//! `support`): each instance's model occupancy (layer/model/occupancy.rs),
//! turned and sized as it is drawn and rounded outward to the tier its size
//! is filled at — whole cells for a thing a cell long or more, the
//! sub-cells of its own size for a smaller one — then folded to the cell
//! with the objects, sub-cell by sub-cell, by the objects' own rule. So a
//! tree, a car or a candle is a volume the way a cube of its size is, and
//! what folds away for a small cube folds away for a small mesh.
//!
//! The instances are the ones z16 draws — the scatter's own candidates and
//! the placed decor, whatever level the view draws there — so what stands
//! where does not change with how far the view is. The triangles stay what
//! is drawn and what a pointer hits (layer/decor/hit.rs).

use std::collections::HashMap;

use pxl_spec::bake::constant::{CELL, CELL_STEPS, OBJECT_FINEST};
use pxl_spec::bake::fold::majority;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_voxel};
use pxl_spec::space::pyramid::chunk_span_of;

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::decor::asset_store::decor_bundle_assets;
use crate::layer::decor::build::kinds;
use crate::layer::decor::cell::{DecorCell, Population};
use crate::layer::decor::hit::group_models;
use crate::layer::decor::instance::DECOR_GROUPS;
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::object::size::tier_of;
use crate::layer::object::store::{Store, VOID};
use crate::shell::setting::store::settings_store;
use crate::world::access::support_revision;

/// What a sub-cell a decor volume fills holds while it is folded: no value
/// the catalogs give out.
const FILLED: u16 = u16::MAX;

/// A cell's sub-cells at the objects' finest, one bit each, x fastest.
type SubCells = [u64; (CELL_STEPS * CELL_STEPS * CELL_STEPS) as usize / 64];

/// An instance as its volume is read: its model, where it stands, its turn
/// and its size.
#[derive(Clone, Copy)]
struct Standing {
    model: u32,
    at: [f64; 3],
    rotation: f64,
    scale: f64,
}

#[derive(Default)]
struct Volume {
    population: Population,
    /// The z16 cells the last ask reached, with what stands in them, as of
    /// `revision`: an ask beside it — the next zone sample, the next step of
    /// a walk — scatters none of them again. Only what one ask reaches is
    /// kept, so it is never more than the few cells an instance's reach
    /// crosses.
    revision: [u32; 3],
    chunks: HashMap<DecorCell, Vec<Standing>>,
    cells: HashMap<i64, SubCells>,
}

static VOLUME: Global<Volume> = Global::new();

/// Changes whenever what decor fills may: a model arriving, or the scatter
/// shown or hidden.
pub fn decor_volume_revision() -> u32 {
    let loaded = decor_bundle_assets().iter().flat_map(|e| &e.models).filter(|m| m.is_some()).count() as u32;
    loaded << 1 | u32::from(settings_store().decor)
}

/// What the cells of column `[ix, iy]` fold to where decor fills any of
/// their sub-cells, as `(cell z, filled)`: true where the fold is a solid —
/// a cube, or decor — false where it is air, and the cell left out where it
/// folds to nothing, for the ground to answer. A cell no decor reaches is
/// left out too, the objects' own fold standing.
pub fn decor_column(store: &Store, ix: i64, iy: i64, out: &mut Vec<(i64, bool)>) {
    out.clear();
    let Some((_, sched)) = kernel().world() else { return };
    let mut models = [None; DECOR_GROUPS];
    for (slot, m) in models.iter_mut().zip(group_models()) {
        *slot = m;
    }
    // How far from where it stands an instance can reach: the largest model
    // at the largest size the scatter draws one, which a placed one, at its
    // model's own size, is not over.
    let reach = models
        .iter()
        .flatten()
        .filter_map(|&m| sched.models.mesh(m))
        .map(|m| m.radius as f64 * m.scale as f64)
        .fold(0.0, f64::max)
        * (1.0 + SCALE_JITTER);
    if reach == 0.0 {
        return;
    }
    let v = VOLUME.get(Volume::default);
    v.cells.clear();
    let revision = support_revision();
    if v.revision != revision {
        v.chunks.clear();
        v.revision = revision;
    }
    let side = CELL.side();
    let steps = CELL_STEPS as i64;
    let fine = OBJECT_FINEST.side();
    let column = [ix, iy].map(|c| [c * steps, (c + 1) * steps]);
    let span = chunk_span_of(0);
    let chunk = |a: f64| (a / span).floor() as i64;
    let [x0, y0] = [ix, iy].map(|c| c as f64 * side - reach);
    let [x1, y1] = [ix, iy].map(|c| (c + 1) as f64 * side + reach);
    let reached = |c: &DecorCell| (chunk(x0)..=chunk(x1)).contains(&c.at[0]) && (chunk(y0)..=chunk(y1)).contains(&c.at[1]);
    v.chunks.retain(|c, _| reached(c));
    for cy in chunk(y0)..=chunk(y1) {
        for cx in chunk(x0)..=chunk(x1) {
            let cell = DecorCell { level: 0, at: [cx, cy] };
            if v.chunks.contains_key(&cell) {
                continue;
            }
            let (ox, oy, _) = cell.rect();
            let Some(built) = v.population.at_z16(cell, kinds(), store) else { continue };
            let standing = built
                .each()
                .filter_map(|(group, inst)| {
                    let model = models.get(group).copied().flatten()?;
                    Some(Standing {
                        model,
                        at: [ox + inst.x as f64, oy + inst.y as f64, inst.z as f64],
                        rotation: inst.rotation as f64,
                        scale: inst.scale as f64 * sched.models.mesh(model)?.scale as f64,
                    })
                })
                .collect();
            v.chunks.insert(cell, standing);
        }
    }
    for standing in v.chunks.values() {
        for &Standing { model, at, rotation, scale: s } in standing {
            let (Some(mesh), Some(occ)) = (sched.models.mesh(model), sched.models.occupancy(model)) else {
                continue;
            };
            // Nothing of it comes near the column.
            let r = mesh.radius as f64 * s;
            if at[0] + r < x0 + reach || at[0] - r > x1 - reach || at[1] + r < y0 + reach || at[1] - r > y1 - reach {
                continue;
            }
            let tier = tier_of(occ.longest as f64 * s);
            let per = (tier / fine).round() as i64;
            let (sn, cs) = rotation.sin_cos();
            let cell = occ.cell as f64;
            let half = cell * s / 2.0 * (cs.abs() + sn.abs());
            // The tier's cells a span `[lo, hi]` touches inside, in the
            // finest's.
            let outward = |lo: f64, hi: f64| [(lo / tier).floor() as i64 * per, (hi / tier).ceil() as i64 * per];
            for r in &occ.runs {
                // The run's column of the model's cells, turned and
                // sized as it is drawn: model (x, z) to world (x, −z)
                // turned by the instance's spin, model Y up.
                let mx = (occ.lo[0] as f64 + (r[0] as f64 + 0.5) * cell) * s;
                let mz = (occ.lo[2] as f64 + (r[1] as f64 + 0.5) * cell) * s;
                let (ux, uy) = (mx, -mz);
                let wx = at[0] + ux * cs - uy * sn;
                let wy = at[1] + ux * sn + uy * cs;
                let zs = [r[2], r[3]].map(|y| at[2] + (occ.lo[1] as f64 + y as f64 * cell) * s);
                let fx = outward(wx - half, wx + half);
                let fy = outward(wy - half, wy + half);
                let fz = outward(zs[0], zs[1]);
                let [fx, fy] = [(fx, column[0]), (fy, column[1])].map(|(f, c)| [f[0].max(c[0]), f[1].min(c[1])]);
                if fx[0] >= fx[1] || fy[0] >= fy[1] {
                    continue;
                }
                for z in fz[0]..fz[1] {
                    let sub = v.cells.entry(z.div_euclid(steps)).or_default();
                    let base = (z.rem_euclid(steps) * steps * steps) as usize;
                    for y in fy[0]..fy[1] {
                        for x in fx[0]..fx[1] {
                            let k = base + ((y - column[1][0]) * steps + (x - column[0][0])) as usize;
                            sub[k / 64] |= 1 << (k % 64);
                        }
                    }
                }
            }
        }
    }
    for (&cz, sub) in &v.cells {
        let n = steps as usize;
        let mut values: Vec<u16> = (0..n * n * n)
            .map(|k| {
                let [x, y, z] = [k % n, k / n % n, k / (n * n)].map(|c| c as i64);
                let own = store.folded_value(OBJECT_FINEST, [column[0][0] + x, column[1][0] + y, cz * steps + z]);
                match (object_value_is_voxel(u32::from(own)), sub[k / 64] >> (k % 64) & 1 != 0) {
                    (true, _) => own,
                    (false, true) => FILLED,
                    (false, false) => VOID as u16,
                }
            })
            .collect();
        // Up to the cell a level at a time, the eight under each parent by
        // the rule the objects fold by (layer/object/store.rs `fold_cell`).
        let mut scale = OBJECT_FINEST;
        let mut m = n;
        while scale < CELL {
            scale = scale.up(1);
            let h = m / 2;
            values = (0..h * h * h)
                .map(|k| {
                    let [x, y, z] = [k % h, k / h % h, k / (h * h)].map(|c| c * 2);
                    let eight: [u16; 8] = core::array::from_fn(|c| values[((z + (c >> 2)) * m + y + (c >> 1 & 1)) * m + x + (c & 1)]);
                    majority(eight, scale.above(CELL))
                })
                .collect();
            m = h;
        }
        match values[0] {
            FILLED => out.push((cz, true)),
            v if object_value_is_voxel(u32::from(v)) => out.push((cz, !object_value_is_air(u32::from(v)))),
            _ => {}
        }
    }
}
