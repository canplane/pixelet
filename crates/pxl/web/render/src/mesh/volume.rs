// pxl-render/mesh/volume.rs

//! The render-time volume for one 64×64 chunk: whole 64³ signed-Z bands that
//! layers paint into and the scan reads back in the same lanes. Up to
//! POOLED_BANDS band stores keep their lanes for reuse, since building them
//! fresh was the heap's worst churn.

use crate::mesh::{grow, try_push, Refused};
use pxl_spec::bake::constant::{band_of, BAND_CELLS};
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::abi::handle::Slots;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free, Block, Guard};

use crate::mesh::voxel_scan::{BandView, ExtraView};

pub const NO_LAYER: u32 = 255;

/// One layer in all four of a box's texture taps.
pub fn taps_of(layer_idx: u8) -> u32 {
    layer_idx as u32 * 0x0101_0101
}

pub const SUB_CELL_NONE: u8 = 0;
const SUB_CELL_SCALE: f64 = 256.0;
/// Read as exactly 1, not 255/256: a coarse box filling one axis while
/// shrinking another is ordinary, and 0 stays free for SUB_CELL_NONE.
const SUB_CELL_FULL: u8 = 255;

pub fn pack_sub_cell_fraction(fraction: f64) -> u32 {
    round(fraction * SUB_CELL_SCALE).clamp(0.0, SUB_CELL_SCALE - 1.0) as u32
}

pub fn unpack_sub_cell_fraction(packed: u8) -> f64 {
    packed as f64 / SUB_CELL_SCALE
}

pub fn unpack_sub_cell_size(packed: u8) -> f64 {
    if packed == SUB_CELL_FULL {
        1.0
    } else {
        packed as f64 / SUB_CELL_SCALE
    }
}

pub struct MaterialClasses;

impl MaterialClasses {
    pub const AIR: u8 = 0;
    pub const LAND: u8 = 1;
    pub const WATER: u8 = 2;
    pub const TRANSPORT: u8 = 3;
    pub const STRUCTURE: u8 = 4;
    /// Exempt from hiding its neighbors' faces, and drawn with its bottom face.
    pub const OBJECT: u8 = 5;
}

/// A voxel's material word, `0xCC_RRGGBB`: its class, then its color.
pub fn pack_material(class: u8, [r, g, b]: [u8; 3]) -> u32 {
    u32::from_be_bytes([class, r, g, b])
}

pub fn unpack_material(word: u32) -> (u8, [u8; 3]) {
    let [class, r, g, b] = word.to_be_bytes();
    (class, [r, g, b])
}

const BAND_AREA: usize = (BAND_CELLS * BAND_CELLS) as usize;
pub const BAND_VOXELS: usize = BAND_AREA * BAND_CELLS as usize;
const MATERIAL_BYTES: u32 = BAND_VOXELS as u32 * 4;
const LAYER_BYTES: u32 = BAND_VOXELS as u32;
const SUB_CELL_BYTES: u32 = BAND_VOXELS as u32 * 4;

/// Column-major: Z is contiguous for paint and scan.
pub fn band_voxel_idx(x: u32, y: u32, z: u32) -> usize {
    (band_col(x, y) * BAND_CELLS + z) as usize
}

/// A band's column, rows of `x` along `y`.
pub fn band_col(x: u32, y: u32) -> u32 {
    y * BAND_CELLS + x
}

pub const POOLED_BANDS: usize = 16;

/// Sub-cell objects sharing a base voxel past the first, sorted by column at seal.
#[derive(Default)]
pub struct Extras {
    pub col: Vec<u32>,
    pub z: Vec<i32>,
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    pub sub: Vec<u32>,
}

impl Extras {
    fn clear(&mut self) {
        self.col.clear();
        self.z.clear();
        self.material.clear();
        self.layer_idx.clear();
        self.sub.clear();
    }

    fn len(&self) -> usize {
        self.col.len()
    }

    /// Lanes are grown before any is written, so a refusal leaves them all as they were.
    fn push(&mut self, col: u32, z: i32, material: u32, layer_idx: u8, sub: u32) -> Result<(), Refused> {
        grow(&mut self.col, 1)?;
        grow(&mut self.z, 1)?;
        grow(&mut self.material, 1)?;
        grow(&mut self.layer_idx, 1)?;
        grow(&mut self.sub, 1)?;
        self.col.push(col);
        self.z.push(z);
        self.material.push(material);
        self.layer_idx.push(layer_idx);
        self.sub.push(sub);
        Ok(())
    }

    /// A later layer replacing the voxel takes its overflow with it.
    fn remove_at(&mut self, col: u32, z: i32) {
        let mut w = 0;
        for i in 0..self.len() {
            if self.col[i] == col && self.z[i] == z {
                continue;
            }
            self.col[w] = self.col[i];
            self.z[w] = self.z[i];
            self.material[w] = self.material[i];
            self.layer_idx[w] = self.layer_idx[i];
            self.sub[w] = self.sub[i];
            w += 1;
        }
        self.col.truncate(w);
        self.z.truncate(w);
        self.material.truncate(w);
        self.layer_idx.truncate(w);
        self.sub.truncate(w);
    }

    /// Stable, so a column's entries keep the order the scan reads them in. The
    /// lanes are permuted in place, a cycle at a time.
    fn sort_by_column(&mut self, order: &mut Vec<u32>) -> Result<(), Refused> {
        let n = self.len();
        order.clear();
        grow(order, n)?;
        order.extend(0..n as u32);
        order.sort_by_key(|&i| self.col[i as usize]);
        for i in 0..n {
            if order[i] as usize == i {
                continue;
            }
            let held = (self.col[i], self.z[i], self.material[i], self.layer_idx[i], self.sub[i]);
            let mut j = i;
            loop {
                let k = order[j] as usize;
                order[j] = j as u32;
                if k == i {
                    (self.col[j], self.z[j], self.material[j], self.layer_idx[j], self.sub[j]) = held;
                    break;
                }
                (self.col[j], self.z[j], self.material[j], self.layer_idx[j], self.sub[j]) = (self.col[k], self.z[k], self.material[k], self.layer_idx[k], self.sub[k]);
                j = k;
            }
        }
        Ok(())
    }
}

/// Chunk-local x and y, absolute z.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell {
    pub cx: u32,
    pub cy: u32,
    pub z: i32,
}

/// The sub-cell word has the size in its top byte; zero for a full cell.
#[derive(Clone, Copy)]
pub struct Voxel {
    pub material: u32,
    pub layer_idx: u8,
    pub sub: u32,
}

struct Store {
    material: usize,
    layer_idx: usize,
    /// Four bytes a voxel, taken only for a band with sub-cell objects.
    sub_cell: Option<Block>,
    extra: Extras,
}

pub struct Band {
    pub band_z: i32,
    store: usize,
}

pub struct Volume {
    pub bands: Vec<Band>,
    pub sealed: bool,
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct VolumeStat {
    pub allocated: u32,
    pub reused: u32,
}

pub struct Volumes {
    stores: Vec<Store>,
    order: Vec<u32>,
    free_stores: Vec<usize>,
    empty_stores: Vec<usize>,
    volumes: Slots<Volume>,
    pub stat: VolumeStat,
}

impl Default for Volumes {
    fn default() -> Self {
        Self::new()
    }
}

impl Volumes {
    pub fn new() -> Volumes {
        Volumes {
            stores: Vec::new(),
            order: Vec::new(),
            free_stores: Vec::new(),
            empty_stores: Vec::new(),
            volumes: Slots::new(),
            stat: VolumeStat::default(),
        }
    }

    pub fn open(&mut self) -> u32 {
        self.volumes.insert(Volume { bands: Vec::new(), sealed: false })
    }

    fn volume(&mut self, id: u32) -> &mut Volume {
        self.volumes.get_mut(id).or_bug(Errno::ENOTRECOVERABLE)
    }

    fn material(&self, store: usize) -> *mut u32 {
        ptr_at::<u32>(self.stores[store].material)
    }

    fn layer(&self, store: usize) -> *mut u8 {
        ptr_at(self.stores[store].layer_idx)
    }

    fn sub_cell(&self, store: usize) -> Option<*mut u8> {
        self.stores[store].sub_cell.map(|(at, _)| ptr_at(at))
    }

    /// None when the heap has no room.
    fn take_store(&mut self) -> Option<usize> {
        if let Some(s) = self.free_stores.pop() {
            self.stat.reused += 1;
            return Some(s);
        }
        let Some(material) = Guard::alloc(MATERIAL_BYTES) else {
            return None;
        };
        let Some((layer_idx, _)) = alloc(LAYER_BYTES) else {
            return None;
        };
        let (material, _) = material.keep();
        self.stat.allocated += 1;
        if let Some(s) = self.empty_stores.pop() {
            let store = &mut self.stores[s];
            store.material = material;
            store.layer_idx = layer_idx;
            return Some(s);
        }
        let store = Store {
            material,
            layer_idx,
            sub_cell: None,
            extra: Extras::default(),
        };
        if try_push(&mut self.stores, store).is_err() {
            // SAFETY: the lanes just taken, which nothing has seen.
            unsafe {
                free((material, MATERIAL_BYTES));
                free((layer_idx, LAYER_BYTES));
            }
            return None;
        }
        Some(self.stores.len() - 1)
    }

    fn get_or_create_band(&mut self, id: u32, band_z: i32) -> Option<usize> {
        xpute_core::ensure!(!self.volume(id).sealed, ENOTRECOVERABLE);
        let bands = &self.volume(id).bands;
        let insert_at = bands.iter().position(|b| b.band_z >= band_z).unwrap_or(bands.len());
        if insert_at < bands.len() && bands[insert_at].band_z == band_z {
            return Some(insert_at);
        }
        grow(&mut self.volume(id).bands, 1).ok()?;
        let store = self.take_store()?;
        // A store keeps whatever its last borrower wrote.
        // SAFETY: the store's lanes are this module's blocks of BAND_VOXELS.
        unsafe {
            core::slice::from_raw_parts_mut(self.material(store), BAND_VOXELS).fill(0);
            core::slice::from_raw_parts_mut(self.layer(store), BAND_VOXELS).fill(NO_LAYER as u8);
        }
        self.drop_sub_cell(store);
        self.stores[store].extra.clear();
        self.volume(id).bands.insert(insert_at, Band { band_z, store });
        Some(insert_at)
    }

    fn drop_sub_cell(&mut self, store: usize) {
        if let Some(block) = self.stores[store].sub_cell.take() {
            // SAFETY: the lane this store took and has not given back.
            unsafe { free(block) };
        }
    }

    /// Zeroed, since zero is SUB_CELL_NONE.
    fn ensure_sub_cell(&mut self, store: usize) -> Option<*mut u8> {
        if self.stores[store].sub_cell.is_none() {
            let (at, bytes) = alloc(SUB_CELL_BYTES)?;
            self.stores[store].sub_cell = Some((at, bytes));
            // SAFETY: a block of `bytes` this module just took.
            unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(at), bytes as usize).fill(0) };
        }
        self.sub_cell(store)
    }

    fn locate(z: i32) -> (i32, u32) {
        band_of(z as i64)
    }

    /// Later layers overwrite earlier ones. False when a band could not be made.
    pub fn set_voxel(&mut self, id: u32, at: Cell, v: Voxel) -> bool {
        let Cell { cx, cy, z } = at;
        // A column past the band would index the next heap block.
        if cx >= BAND_CELLS || cy >= BAND_CELLS {
            return false;
        }
        let Voxel { material, layer_idx, sub } = v;
        let (band_z, local_z) = Self::locate(z);
        let Some(b) = self.get_or_create_band(id, band_z) else { return false };
        let store = self.volume(id).bands[b].store;
        let idx = band_voxel_idx(cx, cy, local_z);
        // SAFETY: lanes of BAND_VOXELS; idx is inside a band.
        unsafe {
            *self.material(store).add(idx) = material;
            *self.layer(store).add(idx) = layer_idx;
        }
        if self.stores[store].extra.len() > 0 {
            self.stores[store].extra.remove_at(band_col(cx, cy), z);
        }
        if sub >> 24 != SUB_CELL_NONE as u32 {
            let Some(lane) = self.ensure_sub_cell(store) else { return false };
            // SAFETY: the lane is BAND_VOXELS * 4 bytes.
            unsafe {
                let at = lane.add(idx * 4);
                *at = sub as u8;
                *at.add(1) = (sub >> 8) as u8;
                *at.add(2) = (sub >> 16) as u8;
                *at.add(3) = (sub >> 24) as u8;
            }
        } else if let Some(lane) = self.sub_cell(store) {
            unsafe { core::slice::from_raw_parts_mut(lane.add(idx * 4), 4).fill(0) };
        }
        true
    }

    pub fn append_sub_voxel(&mut self, id: u32, at: Cell, v: Voxel) -> bool {
        let Cell { cx, cy, z } = at;
        if cx >= BAND_CELLS || cy >= BAND_CELLS {
            return false;
        }
        let Voxel { material, layer_idx, sub } = v;
        let (band_z, _) = Self::locate(z);
        let Some(b) = self.get_or_create_band(id, band_z) else { return false };
        let store = self.volume(id).bands[b].store;
        self.stores[store].extra.push(band_col(cx, cy), z, material, layer_idx, sub).is_ok()
    }

    pub fn fill_column(&mut self, id: u32, column: (u32, u32), z0: i32, z1: i32, v: Voxel) -> bool {
        let (cx, cy) = column;
        if cx >= BAND_CELLS || cy >= BAND_CELLS {
            return false;
        }
        let Voxel { material, layer_idx, .. } = v;
        if z1 < z0 {
            return true;
        }
        let (first, first_z) = Self::locate(z0);
        let (last, last_z) = Self::locate(z1);
        for band_z in first..=last {
            let Some(b) = self.get_or_create_band(id, band_z) else { return false };
            let store = self.volume(id).bands[b].store;
            let local_z0 = if band_z == first { first_z } else { 0 };
            let local_z1 = if band_z == last { last_z } else { BAND_CELLS - 1 };
            let start = band_voxel_idx(cx, cy, local_z0);
            let end = band_voxel_idx(cx, cy, local_z1) + 1;
            // SAFETY: lanes of BAND_VOXELS; start..end is one column of a band.
            unsafe {
                core::slice::from_raw_parts_mut(self.material(store).add(start), end - start).fill(material);
                core::slice::from_raw_parts_mut(self.layer(store).add(start), end - start).fill(layer_idx);
                if let Some(lane) = self.sub_cell(store) {
                    core::slice::from_raw_parts_mut(lane.add(start * 4), (end - start) * 4).fill(0);
                }
            }
        }
        true
    }

    /// Orders every band's extras by column so the scan can find a column's own.
    /// False where the heap refused the sort room.
    pub fn seal(&mut self, id: u32) -> bool {
        self.volume(id).sealed = true;
        let mut order = core::mem::take(&mut self.order);
        let mut sealed = true;
        for i in 0..self.band_count(id) {
            let s = self.volume(id).bands[i].store;
            if self.stores[s].extra.sort_by_column(&mut order).is_err() {
                sealed = false;
                break;
            }
        }
        self.order = order;
        sealed
    }

    pub fn release(&mut self, id: u32) {
        let Some(v) = self.volumes.remove(id) else { return };
        for band in v.bands {
            let s = band.store;
            self.stores[s].extra.clear();
            self.drop_sub_cell(s);
            if self.free_stores.len() < POOLED_BANDS {
                self.free_stores.push(s);
            } else {
                let store = &mut self.stores[s];
                // SAFETY: the lanes this store took and has not given back.
                unsafe {
                    free((store.material, MATERIAL_BYTES));
                    free((store.layer_idx, LAYER_BYTES));
                }
                self.empty_stores.push(s);
            }
        }
    }

    /// # Safety
    /// The views are valid until the volume is painted again or released.
    pub unsafe fn band_view(&self, id: u32, i: usize) -> BandView<'static> {
        let band = &self.volumes.get(id).or_bug(Errno::ENOTRECOVERABLE).bands[i];
        let store = &self.stores[band.store];
        let e = &store.extra;
        let lane = |at: usize, len: usize| core::slice::from_raw_parts(ptr_at::<u8>(at).cast_const(), len);
        let voxels = BAND_VOXELS;
        BandView {
            band_z: band.band_z,
            material: core::slice::from_raw_parts(ptr_at::<u32>(store.material).cast_const(), voxels),
            layer_idx: lane(store.layer_idx, voxels),
            sub_cell: store.sub_cell.map(|(at, _)| lane(at, voxels * 4)),
            extra: ExtraView {
                col: core::slice::from_raw_parts(e.col.as_ptr(), e.col.len()),
                z: core::slice::from_raw_parts(e.z.as_ptr(), e.z.len()),
                material: core::slice::from_raw_parts(e.material.as_ptr(), e.material.len()),
                layer_idx: core::slice::from_raw_parts(e.layer_idx.as_ptr(), e.layer_idx.len()),
                sub: core::slice::from_raw_parts(e.sub.as_ptr(), e.sub.len()),
            },
        }
    }

    pub unsafe fn band_views(&self, id: u32, out: &mut Vec<BandView<'static>>) -> bool {
        out.clear();
        let n = self.band_count(id);
        if grow(out, n).is_err() {
            return false;
        }
        for i in 0..n {
            out.push(self.band_view(id, i));
        }
        true
    }

    pub fn band_count(&self, id: u32) -> usize {
        self.volumes.get(id).map_or(0, |v| v.bands.len())
    }
}
