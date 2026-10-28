// pxl-kernel/domain/layer/terrain/tile.rs

//! Terrain as a layer of the pyramid; a tile's record is a room in
//! `.fixed.terrain` for what is read of it whole, its lines, decoded into
//! straight from the packet. Its cells are its
//! windows' (domain/layer/terrain/window.rs), filled off the packet, which is kept
//! only until the GPU has what they want of it (`kept`). A tile the bake
//! left as no object is drawn from the tile over it, whose cells there are
//! open water; at the source's top, it is void.

use crate::domain::layer::earth::Earth;
use crate::domain::layer::kept::Kept;
use crate::domain::layer::residency::Residency;
use crate::sys::io::param::{BODY_SLOTS, MAX_CREDITS};
use pxl_spec::bake::separation;
use pxl_spec::bake::tile::terrain::{decode_into, TerrainTile};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::mem;
use xpute_guest::mem::heap::Block;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::failure::Failure;

use crate::domain::layer::terrain::fixed::RANGE;
use crate::domain::layer::terrain::fixed::{separation_lane, SEPARATION_LANES};
use crate::domain::layer::Base;
use crate::sys::io::demand::{Phases, Stream, TaskPhaseName};
use pxl_spec::bake::tile::Occupancy;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

static TERRAIN: Global<Residency<TerrainTile>> = Global::new();

impl Base for TerrainTile {
    const STREAM: Stream = Stream::terrain;
    const LATTICE: Lattice = GROUND;
    /// The focus's and a destination's windows' tiles.
    const PINNED: u32 = crate::domain::layer::terrain::window::PINNED_TILES;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn residency() -> &'static mut Residency<TerrainTile> {
        TERRAIN.get(Residency::default)
    }

    fn empty() -> Option<TerrainTile> {
        Some(TerrainTile::void())
    }

    /// One pass from the packet: nothing tile-sized stands between arrival and
    /// drawing.
    fn receive(
        _tile: Tile,
        record: u32,
        pkt: &[u8],
        phases: &mut Phases,
    ) -> Result<TerrainTile, Failure> {
        let t0 = now();
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let terrain = decode_into(pkt, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// What its windows want of it is the GPU's, off the packet: kept only
    /// where a window wants one of its squares, so an ancestor's or another
    /// zoom's packet no window reads takes no place of `most_kept`.
    fn keep(record: Option<u32>, block: Block) {
        let wanted = |r: &u32| {
            TerrainTile::residency()
                .in_frame(*r)
                .is_some_and(crate::domain::layer::terrain::window::is_tile_wanted)
        };
        match record {
            Some(r) if !wanted(&r) => {
                kept().forget(r);
                crate::sys::io::fetch::release(Some(block));
            }
            _ => kept().keep(record, block, most_kept()),
        }
    }
}

/// The most terrain packets kept at once: the body slots (`BODY_SLOTS`)
/// less one for the most credits the host has granted and one for every
/// Earth frame,
/// so the packets kept, which only a drawn frame gives back, never take the
/// slots a read needs to arrive into (sys/io/body.rs). A packet past it is
/// freed once decoded, and its tile asked again when one could be kept
/// (`again_room`, domain/layer/terrain/window.rs `step`).
pub fn most_kept() -> u32 {
    let credits = crate::sys::io::request::most_credits().max(1);
    BODY_SLOTS.saturating_sub(credits + Residency::<Earth>::FRAMES)
}

const _: () = assert!(
    BODY_SLOTS > MAX_CREDITS + Residency::<Earth>::FRAMES,
    "the body slots keep a terrain packet beside a body on every credit and the Earth's"
);

/// How many tiles may be asked again for their packets now: as many as
/// could still be kept, less those asked again and not yet come, so none
/// is fetched only to be freed past `most_kept`.
pub fn again_room() -> u32 {
    // Not one whose object failed: it is not read again while it is wanted.
    let coming = TerrainTile::residency()
        .asked_again(|p| !crate::sys::io::request::has_object_failed(Stream::terrain, p.subject()));
    most_kept().saturating_sub(kept().count() + coming)
}

/// The tiles whose squares the windows want filled, off their packets.
static KEPT: Global<Kept> = Global::new();

pub fn kept() -> &'static mut Kept {
    KEPT.get(|| Kept::new(Residency::<TerrainTile>::FRAMES))
}

/// Whether `tile`'s packet is kept, its squares still to be handed.
pub fn kept_for(tile: Tile) -> bool {
    TerrainTile::residency()
        .frame(tile)
        .is_some_and(|f| kept().waiting(f))
}

/// # Safety
/// The record's lanes are the tile's that is given them, and no other's,
/// from now until another tile takes the record.
unsafe fn room_of(record: u32) -> separation::Room {
    /// Lane `K` of the record, as long as the layout makes it, its element
    /// the room's own and as wide as the layout counts it.
    ///
    /// # Safety
    /// As `room_of`'s.
    unsafe fn lane<T, const K: usize>(record: u32) -> &'static mut [T] {
        const {
            assert!(
                SEPARATION_LANES[K].1 == size_of::<T>(),
                "a lane's element as wide as the layout counts it"
            )
        };
        let (at, bytes) = separation_lane(record, K);
        // Inside the range by the layout, so past its end is a bug.
        let at = RANGE.get().at(at, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
        // SAFETY: the lane is kernel memory at its own size, never freed, and
        // the caller's contract makes it this tile's alone.
        unsafe { core::slice::from_raw_parts_mut(mem::at::<T>(at), SEPARATION_LANES[K].0) }
    }
    // SAFETY: as this function's.
    unsafe {
        separation::Room {
            starts: lane::<_, 0>(record),
            points: lane::<_, 1>(record),
            tops: lane::<_, 2>(record),
            width: lane::<_, 3>(record),
            kind: lane::<_, 4>(record),
            form: lane::<_, 5>(record),
            clear: lane::<_, 6>(record),
        }
    }
}
