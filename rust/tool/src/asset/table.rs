// pxl-tool/asset/table.rs

//! The ingested-asset table, written as the JSON the spec renders from.

use std::fmt::Write;

use crate::asset::pack::Slot;
use crate::spec::mark;

pub struct Row {
    pub path: String,
    pub key: String,
    pub bytes: u64,
    pub extent: Option<[f64; 3]>,
}

pub struct Pack {
    /// The path its image ships at, a row of the table (`cube.json5` `pack`).
    pub image: String,
    pub cell: u32,
    pub columns: u32,
    pub tiles: Vec<String>,
    pub means: Vec<[u8; 3]>,
    pub faces: Vec<TileFace>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Top,
    Side,
    Bottom,
}

impl Face {
    /// Every face, in the pack's order: the members of the CubeFace the
    /// specs' ingest writes (spec/ingest.rs).
    pub const ALL: [Face; 3] = [Face::Top, Face::Side, Face::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Face::Top => "top",
            Face::Side => "side",
            Face::Bottom => "bottom",
        }
    }
}

pub struct TileFace {
    /// The catalog's group, `ground` or `object`.
    pub group: String,
    pub ty: String,
    pub face: Face,
    pub(crate) tile: u32,
}

/// Each slot and the tile it wears; one tile may be several slots'. A slot
/// naming no catalog type is an error.
pub fn faces(
    slots: &[(u32, Slot)],
    groups: &[(String, Vec<String>)],
) -> Result<Vec<TileFace>, String> {
    let mut out = Vec::new();
    for (n, slot) in slots {
        let (face, ty, wanted) = match slot {
            Slot::Top { group, ty } => (Face::Top, ty, Some(group.as_str())),
            Slot::Side { ty } => (Face::Side, ty, None),
            Slot::Bottom { ty } => (Face::Bottom, ty, None),
        };
        let before = out.len();
        for (group, types) in groups
            .iter()
            .filter(|(g, _)| wanted.is_none_or(|w| w == *g))
        {
            if types.contains(ty) {
                out.push(TileFace {
                    group: group.clone(),
                    ty: ty.clone(),
                    face,
                    tile: *n,
                });
            }
        }
        if out.len() == before {
            return Err(format!(
                "the manifest's {slot:?} is no cube in spec/spec/asset/cube.json5"
            ));
        }
    }
    Ok(out)
}

/// The table as `spec/spec/asset/ingest.json5`, laid out as prettier lays it
/// out, which `cargo spec` renders into the host's table, the pack's layout
/// and the catalog (pxl-tool spec/ingest.rs, spec/catalog.rs).
pub fn render_json(rows: &[Row], pack: &Pack, symbols: &[String]) -> String {
    let string = |s: &str| format!("{s:?}");
    let doc = [
        mark("asset/content/", "pxl-tool bin/asset.rs").as_str(),
        "Every shipped file by its path under asset/content/, in the ingest's sorted",
        "order, which numbers it after ASSET from 1: its bucket key (the",
        "SHA-256 of its bytes, which where it lies does not change), its size, and for",
        "a model its extent in ENU world units, measured from the glb. Then the",
        "texture pack, a file of the table: `columns` tiles of `cell` pixels per row,",
        "tile n at column n % columns and row n / columns, `tiles` the path each",
        "holds, `means` each one's mean color in linear light, and `faces` the cube",
        "face — group, type, face — each tile is. Then Carto's place symbols, which",
        "the client carries in its binary: each by its path as Carto gives it, in the",
        "order of their masks, which numbers each from 1.",
    ]
    .join("\n");
    // A token whose value is a list, one item a line, under `key`, at `pad`.
    let list = |out: &mut String, pad: &str, key: &str, items: Vec<String>| {
        let _ = writeln!(out, "{pad}{key}: {{\n{pad}  $value: [");
        for i in items {
            let _ = writeln!(out, "{pad}    {i},");
        }
        let _ = writeln!(out, "{pad}  ],\n{pad}}},");
    };
    let mut out = String::with_capacity(rows.len() * 200);
    let _ = writeln!(out, "{{\n  $description: {},\n  files: {{\n    $value: [", string(&doc));
    for r in rows {
        let _ = writeln!(out, "      {{\n        path: {},\n        key: {},\n        bytes: {},", string(&r.path), string(&r.key), r.bytes);
        if let Some(e) = r.extent {
            let _ = writeln!(out, "        extent: [{}, {}, {}],", e[0], e[1], e[2]);
        }
        out.push_str("      },\n");
    }
    out.push_str("    ],\n  },\n  pack: {\n");
    let _ = writeln!(out, "    image: {{ $value: {} }},", string(&pack.image));
    let _ = writeln!(out, "    cell: {{ $value: {} }},", pack.cell);
    let _ = writeln!(out, "    columns: {{ $value: {} }},", pack.columns);
    list(&mut out, "    ", "tiles", pack.tiles.iter().map(|t| string(t)).collect());
    list(&mut out, "    ", "means", pack.means.iter().map(|m| format!("[{}, {}, {}]", m[0], m[1], m[2])).collect());
    list(
        &mut out,
        "    ",
        "faces",
        pack.faces
            .iter()
            .map(|f| format!("[{}, {}, {}, {}]", string(&f.group), string(&f.ty), string(f.face.name()), f.tile))
            .collect(),
    );
    out.push_str("  },\n");
    list(&mut out, "  ", "symbols", symbols.iter().map(|s| string(s)).collect());
    out.push_str("}\n");
    out
}
