// pxl_shared/spec/cell/type.rs

use std::collections::BTreeMap;

pub struct ColorJsonEntry {
    /// How the colour is written in the JSON, which is how a person picks one.
    pub hex: &'static str,
    /// The same colour as the one number it is, packed `0xRRGGBB`, worked out
    /// by the generator so nothing here parses text to find a colour out.
    pub rgb: u32,
    pub note: Option<&'static str>,
}

pub struct ColorJsonMeta {
    pub description: Option<&'static str>,
    pub source: Option<&'static str>,
}

/// The colour file: its `_meta`, then its channels in the file's order —
/// the order `Object.keys` walks there.
pub struct ColorJsonFile {
    pub _meta: Option<ColorJsonMeta>,
    pub channels: &'static [(&'static str, &'static [(&'static str, ColorJsonEntry)])],
}

/// A catalog colour, in both the forms anyone wants it: as written, for a
/// record a person reads, and packed, for everything that has to light or
/// blend with it. One table carries both because they are one colour — a
/// second table keyed the same way is how they come to disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub hex: &'static str,
    pub rgb: u32,
}

impl Swatch {
    pub const fn new(hex: &'static str, rgb: u32) -> Swatch {
        Swatch { hex, rgb }
    }
}

/// One channel's colours by type id. A `Record<number, …>` walks its
/// integer keys ascending, which is a BTreeMap's order.
pub type ChannelColors = BTreeMap<u32, Option<Swatch>>;

/// Channels' colours, by channel name, in the order they were built.
#[derive(Clone, Default)]
pub struct Colors(pub Vec<(&'static str, ChannelColors)>);

impl Colors {
    /// `colors[channel]`.
    pub fn get(&self, channel: &str) -> Option<&ChannelColors> {
        self.0.iter().find(|(name, _)| *name == channel).map(|(_, c)| c)
    }
}

/// A catalog's channels: each a name and its types, in declaration order.
pub type Channels = [(&'static str, &'static [(&'static str, u8)])];

pub fn build_colors_from_json(types: &Channels, json: &ColorJsonFile, seed: &ChannelColors) -> Colors {
    let mut out = Colors::default();

    for (ch_key, ch_entries) in json.channels {
        let Some((_, types_for_ch)) = types.iter().find(|(name, _)| name == ch_key) else {
            panic!("color JSON references unknown channel \"{ch_key}\"");
        };

        let mut ch_out = seed.clone();
        for (type_name, entry) in ch_entries.iter() {
            let Some((_, type_id)) = types_for_ch.iter().find(|(name, _)| name == type_name) else {
                panic!("color JSON references unknown type \"{ch_key}.{type_name}\"");
            };
            ch_out.insert(*type_id as u32, Some(Swatch::new(entry.hex, entry.rgb)));
        }

        out.0.push((ch_key, ch_out));
    }

    out
}
