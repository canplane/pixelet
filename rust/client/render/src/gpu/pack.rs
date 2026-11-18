// pxl-render/gpu/pack.rs

//! The texture pack as one 2D array texture with its mip chain (`mip.rs`), and the table that
//! maps a layer's top, side and bottom to slots of the array. Each slot is a distinct tile. A
//! face with no tile of its own shares the top's slot, and a layer the pack ships nothing for
//! takes no slot and reads as the box's color.

use crate::frame::shader::word::{IMAGE_BOTTOM, IMAGE_SIDE, IMAGE_TOP, NO_SLOT, SLOT_BITS};
use pxl_spec::asset::cube::ALL_CUBES;
use pxl_spec::asset::ingest::{CubeFace, PACK};
use pxl_spec::asset::{TileOrigin, shipped_sentinel_tile, shipped_texture_tile};

use pxl_gpu_link::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{Format, Gpu, Image, buffer_usage, bytes, place, release, texture_usage};
use crate::gpu::mip::{MipPass, mip_cnt};
use crate::paint::palette::{LAYER_CNT, SENTINELS};

pub const LAYER_TEX_SIZE: u32 = PACK.cell;

const fn slot_word(top: u32, side: u32, bottom: u32) -> u32 {
    top << (IMAGE_TOP * SLOT_BITS)
        | side << (IMAGE_SIDE * SLOT_BITS)
        | bottom << (IMAGE_BOTTOM * SLOT_BITS)
}

const NO_SLOT_WORD: u32 = slot_word(NO_SLOT, NO_SLOT, NO_SLOT);

fn layout() -> (Vec<TileOrigin>, Vec<u32>) {
    fn slot_of(slots: &mut Vec<TileOrigin>, tile: Option<TileOrigin>) -> u32 {
        let Some(tile) = tile else { return NO_SLOT };
        match slots.iter().position(|s| *s == tile) {
            Some(i) => i as u32,
            None => {
                slots.push(tile);
                (slots.len() - 1) as u32
            }
        }
    }
    let mut slots: Vec<TileOrigin> = Vec::new();
    let table = ALL_CUBES
        .iter()
        .map(|&cube| {
            // An asset wears what the pack ships for it: its own side or bottom where it has
            // one, and otherwise its top's image on every face, as a `cube_all` model does. The
            // dirt under a face is an asset of its own, the stratum below (pxl-engine ground.rs
            // `Cell::asset`), and never a side's image.
            let top = shipped_texture_tile(cube, CubeFace::Top);
            let side = shipped_texture_tile(cube, CubeFace::Side).or(top);
            let bottom = shipped_texture_tile(cube, CubeFace::Bottom).or(top);
            slot_word(
                slot_of(&mut slots, top),
                slot_of(&mut slots, side),
                slot_of(&mut slots, bottom),
            )
        })
        .collect::<Vec<u32>>();
    // A sentinel drawn wears its one tile on every face.
    let table = table
        .into_iter()
        .chain(SENTINELS.iter().map(|&s| {
            let n = slot_of(&mut slots, shipped_sentinel_tile(s));
            slot_word(n, n, n)
        }))
        .collect();
    (slots, table)
}

pub struct Pack {
    tex: u32,
    pub view: u32,
    pub table: u32,
    pub sampler: u32,
    slot_cnt: u32,
    mip_cnt: u32,
    loaded: bool,
}

impl Pack {
    /// Until the pack arrives, every face reads `NO_SLOT`.
    pub fn new(g: &Gpu) -> Pack {
        let (tiles, _) = layout();
        let slot_cnt = tiles.len() as u32;
        let mip_cnt = mip_cnt(LAYER_TEX_SIZE, LAYER_TEX_SIZE);
        let usage = texture_usage::TEXTURE_BINDING
            | texture_usage::COPY_DST
            | texture_usage::RENDER_ATTACHMENT;
        let tex = g.texture(
            LAYER_TEX_SIZE,
            LAYER_TEX_SIZE,
            slot_cnt,
            Format::Rgba8unormSrgb,
            mip_cnt,
            usage,
        );
        let view = g.view(tex, true, 0, mip_cnt, 0, slot_cnt);
        // The sampler filters anisotropically, at 4x, as the ground is mostly seen at a
        // grazing angle, where an isotropic mip blurs the tile along the view.
        let sampler = g.sampler(
            FilterMode::Linear,
            FilterMode::Linear,
            MipmapFilterMode::Linear,
            AddressMode::Repeat,
            None,
            4,
        );
        let blank: Vec<u32> = vec![NO_SLOT_WORD; LAYER_CNT];
        let table = g.buf(
            (blank.len() * 4) as u32,
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buf(table, 0, bytes(&blank));
        Pack {
            tex,
            view,
            table,
            sampler,
            slot_cnt,
            mip_cnt,
            loaded: false,
        }
    }

    /// Copies the pack's tiles into their slots, builds their mips and writes the table, the
    /// first time the pack arrives. Returns whether it arrived this frame.
    pub fn step(&mut self, g: &Gpu, mip: &MipPass, pack: Option<Image>) -> bool {
        if self.loaded {
            return false;
        }
        let Some(texture) = pack.and_then(|(image, file)| g.image(image, file)) else {
            return false;
        };
        let (tiles, table) = layout();
        let encoder = g.encoder();
        for (slot, tile) in tiles.iter().enumerate() {
            g.copy_texture(
                encoder,
                &place(texture, [tile.x, tile.y, 0]),
                &place(self.tex, [0, 0, slot as u32]),
                [LAYER_TEX_SIZE, LAYER_TEX_SIZE, 1],
            );
        }
        g.submit(encoder);
        release(texture);
        mip.build(g, self.tex, self.mip_cnt, 0, self.slot_cnt);
        g.write_buf(self.table, 0, bytes(&table));
        self.loaded = true;
        true
    }
}
