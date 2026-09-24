// pxl-render/mesh/paint/object/adapter.rs

//! The paint that turns the object store's records into voxels of a chunk
//! volume: what each state paints as, the eraser's carve preview, and a
//! column's entries painted band by band.

use std::collections::BTreeSet;

use pxl_spec::bake::constant::{BandZ, CellZ, BAND_CELLS, BAND_UNITS, MIN_LEVEL, POS_BITS, POS_MASK, STEP_UNITS, U_PER_CELL};
use pxl_spec::cell::cube_types;
use xpute_core::math::scalar::round;

use crate::mesh::volume::{pack_sub_cell_fraction, Cell, Volumes, Voxel, NO_LAYER, SUB_CELL_NONE};

const AIR: u32 = cube_types::common::air as u32;

/// A column's entries as the object store lays them out, one lane a field:
/// each entry's band, its packed position in the band, and its state.
#[derive(Clone, Copy)]
pub struct Entries<'a> {
    pub band: &'a [BandZ],
    pub pos: &'a [u32],
    pub state: &'a [u32],
}

/// What a state paints as: nothing (decor renders as instanced geometry,
/// not a voxel), air, or a solid with a material.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Untaught = 0,
    Solid = 1,
    Air = 2,
    Decor = 3,
}

/// The one entry that is not stored — the eraser's carve preview, at a
/// corner relative to the column's base, in world units.
#[derive(Clone, Copy)]
pub struct Carve {
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
}

/// What a paint made: whether any voxel was set, and whether one was refused.
#[derive(Clone, Copy)]
pub struct Paint {
    pub any: bool,
    pub refused: bool,
}

/// Where a step of a paint got to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Painted {
    /// The entry the next step starts at.
    More(usize),
    Done,
    Refused,
}

/// A column's entries painted as voxels of a chunk volume.
pub struct Painter {
    kinds: Vec<Kind>,
    materials: Vec<u32>,
    layers: Vec<u8>,
    /// The voxels this paint has claimed, so a second sub-cell object
    /// landing on a claimed one is filed as overflow rather than
    /// overwriting it; by packed address, cleared per paint.
    claimed: BTreeSet<u32>,
}

/// What a cell's Z is shifted by to address it unsigned: PVF3 gives Z sixteen
/// bits, so this is half that range and the sum is one.
const Z_BIAS: CellZ = 1 << 15;

/// A voxel's address within a paint: its Z above the bias, then its cell in
/// the band. Sixteen bits of Z over twelve of column is twenty-eight, which is
/// what the claimed set is keyed by.
fn voxel_address(cx: u32, cy: u32, z: CellZ) -> u32 {
    (((z + Z_BIAS) as u32) << 12) | (cy << 6) | cx
}

impl Default for Painter {
    fn default() -> Self {
        Painter::new()
    }
}

impl Painter {
    pub fn new() -> Painter {
        Painter {
            kinds: Vec::new(),
            materials: Vec::new(),
            layers: Vec::new(),
            claimed: BTreeSet::new(),
        }
    }

    /// Teaches what `state` paints as. A state is what a leaf's palette
    /// index resolves to, so a state past a leaf's width is a bug in the
    /// catalogue and not taught, rather than sizing the tables to it.
    pub fn teach(&mut self, state: u32, kind: Kind, material: u32, layer: u8) {
        if state >= u16::MAX as u32 {
            return;
        }
        let i = state as usize;
        if i >= self.kinds.len() {
            self.kinds.resize(i + 1, Kind::Untaught);
            self.materials.resize(i + 1, 0);
            self.layers.resize(i + 1, NO_LAYER as u8);
        }
        self.kinds[i] = kind;
        self.materials[i] = material;
        self.layers[i] = layer;
    }

    /// What `state` paints as: its packed material and layer, or none when
    /// it paints nothing (untaught, air, decor).
    pub fn material_of(&self, state: u32) -> Option<(u32, u8)> {
        match self.kind(state) {
            Kind::Solid => Some((self.materials[state as usize], self.layers[state as usize])),
            _ => None,
        }
    }

    /// Whether `state` is a voxel — a solid or air — rather than an instance
    /// or nothing.
    pub fn is_voxel(&self, state: u32) -> bool {
        matches!(self.kind(state), Kind::Solid | Kind::Air)
    }

    fn kind(&self, state: u32) -> Kind {
        self.kinds.get(state as usize).copied().unwrap_or(Kind::Untaught)
    }

    /// Paints a column's `entries` into `volume`: every one but the one
    /// hidden (by its band and packed position), then the carve preview.
    /// Whether any voxel was painted, and whether a paint was refused — after
    /// which the rest is not painted, as the volume is not the composite that
    /// was asked for anyway.
    pub fn paint(&mut self, entries: Option<Entries>, volumes: &mut Volumes, volume: usize, hide: Option<(i32, u32)>, carve: Option<Carve>) -> Paint {
        let mut claimed = core::mem::take(&mut self.claimed);
        claimed.clear();
        let mut any = false;
        let mut refused = matches!(self.paint_entries(entries, volumes, volume, hide, 0, usize::MAX, &mut claimed, &mut any), Painted::Refused);
        if !refused {
            refused = !self.paint_carve(volumes, volume, carve, &mut claimed, &mut any);
        }
        self.claimed = claimed;
        Paint { any, refused }
    }

    /// Paints a column's `entries` from the `from`th, until the voxels set
    /// pass `voxels`, with the claims of the paint they continue: a paint
    /// cut into steps, each bounded by `voxels` and one entry past it — an
    /// entry sets at most a band's cells. The carve preview is not an entry;
    /// `paint_carve` ends the paint.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_entries(
        &self,
        entries: Option<Entries>,
        volumes: &mut Volumes,
        volume: usize,
        hide: Option<(i32, u32)>,
        from: usize,
        voxels: usize,
        claimed: &mut BTreeSet<u32>,
        any: &mut bool,
    ) -> Painted {
        let Some(e) = entries else { return Painted::Done };
        let mut set = 0;
        for k in from..e.band.len() {
            if set >= voxels {
                return Painted::More(k);
            }
            let (band, pos, state) = (e.band[k], e.pos[k], e.state[k]);
            if hide == Some((band, pos)) {
                continue;
            }
            let x = pos & POS_MASK;
            let y = (pos >> POS_BITS) & POS_MASK;
            let z = (pos >> (2 * POS_BITS)) & POS_MASK;
            let level = (pos >> (3 * POS_BITS)) as i32 + MIN_LEVEL;
            let lx = x as f64 * STEP_UNITS;
            let ly = y as f64 * STEP_UNITS;
            let lz = band as f64 * BAND_UNITS + z as f64 * STEP_UNITS;
            let size = (2.0f64).powi(level);
            match self.paint_entry(volumes, volume, lx, ly, lz, size, state, claimed, any) {
                Some(n) => set += n + 1,
                None => return Painted::Refused,
            }
        }
        Painted::Done
    }

    /// The carve preview painted as air, last. False when a paint was refused.
    pub fn paint_carve(&self, volumes: &mut Volumes, volume: usize, carve: Option<Carve>, claimed: &mut BTreeSet<u32>, any: &mut bool) -> bool {
        match carve {
            Some(c) => self.paint_entry(volumes, volume, c.lx, c.ly, c.lz, c.size, AIR, claimed, any).is_some(),
            None => true,
        }
    }

    /// One entry at a corner relative to the column's base: the voxels it
    /// set, or none when a paint was refused.
    #[allow(clippy::too_many_arguments)]
    fn paint_entry(&self, volumes: &mut Volumes, volume: usize, lx: f64, ly: f64, lz: f64, size: f64, state: u32, claimed: &mut BTreeSet<u32>, any: &mut bool) -> Option<usize> {
        let kind = self.kind(state);
        if kind == Kind::Decor || kind == Kind::Untaught {
            return Some(0);
        }
        let is_sub_cell = size < U_PER_CELL;
        let cell_of = |v: f64| if is_sub_cell { (v / U_PER_CELL).floor() } else { round(v / U_PER_CELL) };
        let cx0 = cell_of(lx).max(0.0) as u32;
        let cy0 = cell_of(ly).max(0.0) as u32;
        let z = cell_of(lz) as i32;
        if cx0 >= BAND_CELLS || cy0 >= BAND_CELLS {
            return Some(0);
        }
        let sz_cells = (round(size / U_PER_CELL) as u32).max(1);
        let cx1 = (cx0 + sz_cells).min(BAND_CELLS);
        let cy1 = (cy0 + sz_cells).min(BAND_CELLS);
        let is_air = kind == Kind::Air;
        let (material, layer_idx) = if is_air {
            (0, NO_LAYER as u8)
        } else {
            (self.materials[state as usize], self.layers[state as usize])
        };

        let fraction = |v: f64| if is_sub_cell { pack_sub_cell_fraction(v) } else { 0 };
        let sub_sz = if is_sub_cell { pack_sub_cell_fraction(size / U_PER_CELL) } else { SUB_CELL_NONE as u32 };
        let sub_ox = fraction((lx - cx0 as f64 * U_PER_CELL) / U_PER_CELL);
        let sub_oy = fraction((ly - cy0 as f64 * U_PER_CELL) / U_PER_CELL);
        let sub_oz = fraction((lz - z as f64 * U_PER_CELL) / U_PER_CELL);
        let sub = sub_ox | (sub_oy << 8) | (sub_oz << 16) | (sub_sz << 24);
        let v = Voxel { material, layer_idx, sub };

        for cy in cy0..cy1 {
            for cx in cx0..cx1 {
                let address = voxel_address(cx, cy, z);
                *any = true;
                let at = Cell { cx, cy, z };
                let ok = if claimed.contains(&address) && !is_air {
                    volumes.append_sub_voxel(volume, at, v)
                } else {
                    claimed.insert(address);
                    volumes.set_voxel(volume, at, v)
                };
                if !ok {
                    return None;
                }
            }
        }
        Some(((cx1 - cx0) * (cy1 - cy0)) as usize)
    }
}
