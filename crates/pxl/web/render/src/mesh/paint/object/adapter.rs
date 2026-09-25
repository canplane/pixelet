// pxl-render/mesh/paint/object/adapter.rs

//! The paint that turns the object store's records into voxels of a chunk
//! volume: what each state paints as, the eraser's carve preview, and a
//! column's entries painted band by band.

use std::collections::BTreeSet;

use pxl_spec::bake::constant::{band_cell_z, in_cell, unpack_pos, BandZ, CellZ, CELL, CELL_STEPS};
use pxl_spec::cell::cube_types;

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

/// The one entry that is not stored — the eraser's carve preview, as the
/// band and packed position the store would file it under.
#[derive(Clone, Copy)]
pub struct Carve {
    pub band: BandZ,
    pub pos: u32,
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
    claimed: Claims,
}

/// The voxels a paint has claimed, so a second sub-cell object landing on a
/// claimed one is filed as overflow rather than overwriting it: by the cell
/// the volume addresses it by, cleared per paint.
pub type Claims = BTreeSet<Cell>;

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
    pub fn paint(&mut self, entries: Option<Entries>, volumes: &mut Volumes, volume: u32, hide: Option<(i32, u32)>, carve: Option<Carve>) -> Paint {
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
    /// entry sets one voxel. The carve preview is not an entry;
    /// `paint_carve` ends the paint.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_entries(&self, entries: Option<Entries>, volumes: &mut Volumes, volume: u32, hide: Option<(i32, u32)>, from: usize, voxels: usize, claimed: &mut Claims, any: &mut bool) -> Painted {
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
            match self.paint_entry(volumes, volume, band, pos, state, claimed, any) {
                Some(n) => set += n + 1,
                None => return Painted::Refused,
            }
        }
        Painted::Done
    }

    /// The carve preview painted as air, last. False when a paint was refused.
    pub fn paint_carve(&self, volumes: &mut Volumes, volume: u32, carve: Option<Carve>, claimed: &mut Claims, any: &mut bool) -> bool {
        match carve {
            Some(c) => self.paint_entry(volumes, volume, c.band, c.pos, AIR, claimed, any).is_some(),
            None => true,
        }
    }

    /// One entry: the voxel it set, or none when a paint was refused. An
    /// entry is at most a cell (the store splits a larger leaf), so it is one
    /// voxel, and what is finer than a cell rides in the voxel's sub-cell
    /// fraction.
    #[allow(clippy::too_many_arguments)]
    fn paint_entry(&self, volumes: &mut Volumes, volume: u32, band: BandZ, pos: u32, state: u32, claimed: &mut Claims, any: &mut bool) -> Option<usize> {
        let kind = self.kind(state);
        if kind == Kind::Decor || kind == Kind::Untaught {
            return Some(0);
        }
        let (x, y, z, scale) = unpack_pos(pos);
        let ([cx, cy, cz], into) = in_cell([x, y, z]);
        let z = band_cell_z(band) as CellZ + cz as CellZ;
        let is_air = kind == Kind::Air;
        let (material, layer_idx) = if is_air {
            (0, NO_LAYER as u8)
        } else {
            (self.materials[state as usize], self.layers[state as usize])
        };

        let sub = if scale < CELL {
            let [ox, oy, oz] = into.map(|v| pack_sub_cell_fraction(v as f64 / CELL_STEPS as f64));
            let sz = pack_sub_cell_fraction(1.0 / scale.count_in(CELL) as f64);
            ox | (oy << 8) | (oz << 16) | (sz << 24)
        } else {
            (SUB_CELL_NONE as u32) << 24
        };
        let v = Voxel { material, layer_idx, sub };

        *any = true;
        let at = Cell { cx, cy, z };
        let ok = if claimed.contains(&at) && !is_air {
            volumes.append_sub_voxel(volume, at, v)
        } else {
            claimed.insert(at);
            volumes.set_voxel(volume, at, v)
        };
        ok.then_some(1)
    }
}
