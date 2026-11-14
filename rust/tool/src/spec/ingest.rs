// pxl-tool/spec/ingest.rs

//! What the asset ingest wrote (`spec/spec/asset/ingest.json5`), rendered:
//! the files, each by its number with every field on both sides, which binds
//! a file's number to its bucket key, and the texture pack's layout, which the
//! renderer reads. A path is the specs' and the tools' name for a file; what
//! is generated elsewhere names it by its number (`file_number`), and Carto's
//! symbols by theirs (`symbol_number`).

use std::fmt::Write;
use std::sync::OnceLock;

use xpute_gen::spec::data::{table_outputs, DataSpec};
use xpute_gen::spec::rs_enum;

use super::asset::{cube_member, cube_number, members};
use super::{description, file, marks, read, u32_of, Node, Output};
use crate::asset::table::Face;
use crate::root;

pub(crate) const INGEST_JSON: &str = "spec/spec/asset/ingest.json5";

pub struct File {
    pub path: String,
    /// ENU (x, y, z-up); None for an image.
    pub extent: Option<[f64; 3]>,
}

/// Every file, in the order that numbers it after ASSET from 1.
pub fn files() -> Result<Vec<File>, String> {
    let spec = read(INGEST_JSON)?;
    let rows = spec
        .get("files")
        .map(Node::val)
        .and_then(Node::arr)
        .ok_or(format!("{INGEST_JSON}: no files"))?;
    rows.iter()
        .map(|r| {
            let at = |field: &str| format!("{INGEST_JSON}: a file's {field}");
            let extent = match r.get("extent").map(Node::val).and_then(Node::arr) {
                None => None,
                Some(e) => Some(
                    e.iter()
                        .map(Node::num)
                        .collect::<Option<Vec<_>>>()
                        .and_then(|e| e.try_into().ok())
                        .ok_or(at("extent"))?,
                ),
            };
            Ok(File {
                path: r.str_at("path").ok_or(at("path"))?.to_string(),
                extent,
            })
        })
        .collect()
}

/// `path`'s number after ASSET, from 1.
pub(crate) fn file_number(path: &str) -> Result<u32, String> {
    static PATHS: OnceLock<Result<Vec<String>, String>> = OnceLock::new();
    let paths = PATHS
        .get_or_init(|| Ok(files()?.into_iter().map(|f| f.path).collect()))
        .as_ref()
        .map_err(Clone::clone)?;
    paths
        .iter()
        .position(|p| p == path)
        .map(|i| i as u32 + 1)
        .ok_or(format!(
            "{path} is no file of {INGEST_JSON}: run the asset ingest"
        ))
}

/// The symbol at `path` (as Carto gives it), by its place among those the
/// ingest rasterized, from 1.
pub(crate) fn symbol_number(path: &str) -> Result<u16, String> {
    static PATHS: OnceLock<Result<Vec<String>, String>> = OnceLock::new();
    let paths = PATHS
        .get_or_init(|| {
            read(INGEST_JSON)?
                .get("symbols")
                .and_then(Node::arr)
                .and_then(|s| s.iter().map(|p| p.str().map(str::to_string)).collect())
                .ok_or(format!("{INGEST_JSON}: no symbols"))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    paths
        .iter()
        .position(|p| p == path)
        .map(|i| i as u16 + 1)
        .ok_or(format!(
            "{path} is no symbol of {INGEST_JSON}: run the asset ingest"
        ))
}

struct Pack {
    image: String,
    cell: u32,
    columns: u32,
    tiles: Vec<String>,
    means: Vec<[u32; 3]>,
    /// Group, type, face, tile.
    faces: Vec<(String, String, String, u32)>,
}

fn pack() -> Result<Pack, String> {
    let spec = read(INGEST_JSON)?;
    let p = spec.get("pack").ok_or(format!("{INGEST_JSON}: no pack"))?;
    let at = |field: &str| format!("{INGEST_JSON}: pack.{field}");
    let list = |field: &str| {
        p.get(field)
            .map(Node::val)
            .and_then(Node::arr)
            .ok_or(at(field))
    };
    let means = list("means")?
        .iter()
        .map(|m| {
            m.arr()
                .and_then(|m| m.iter().map(u32_of).collect::<Option<Vec<_>>>())
                .and_then(|m| m.try_into().ok())
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(at("means"))?;
    let faces = list("faces")?
        .iter()
        .map(|f| match f.arr()? {
            [g, t, face, n] => Some((
                g.str()?.to_string(),
                t.str()?.to_string(),
                face.str()?.to_string(),
                u32_of(n)?,
            )),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(at("faces"))?;
    Ok(Pack {
        image: p.str_at("image").ok_or(at("image"))?.to_string(),
        cell: p.get("cell").and_then(u32_of).ok_or(at("cell"))?,
        columns: p.get("columns").and_then(u32_of).ok_or(at("columns"))?,
        tiles: list("tiles")?
            .iter()
            .map(|t| t.str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or(at("tiles"))?,
        means,
        faces,
    })
}

/// The files, written by xpute's generator from the ingest's own `files`:
/// every field of each file on both sides. The pack's layout follows, the
/// tool's own, as each side reads it.
pub fn outputs() -> Result<Vec<Output>, String> {
    let (spec, pack) = (read(INGEST_JSON)?, pack()?);
    let files = spec
        .get("files")
        .filter(|f| f.is_wrapper())
        .ok_or(format!("{INGEST_JSON}: no files"))?;
    let table = Node::Obj(vec![
        (
            "$description".into(),
            description(&[
                "Every shipped asset by its number after ASSET, file n at `ASSET_FILES[n - 1]`",
                "(ts/client/src/service/sys/fs/asset.ts), and the texture pack as the asset ingest",
                "composed it.",
            ]),
        ),
        (
            "ASSET_FILES".into(),
            Node::Obj(vec![
                (
                    "$description".into(),
                    description(&[
                        "Each file: its path below asset/content/, the specs' and the tools' name for it;",
                        "its bucket key, the SHA-256 of its bytes, which where it lies does not change;",
                        "its size; and for a model its extent in ENU world units (x, y, z-up), measured",
                        "from the glb, null for an image. The bytes themselves are in the bucket.",
                    ]),
                ),
                marks(vec![("row".into(), Node::Str("asset_file".into()))]),
            ]
            .into_iter()
            .chain(files.entries().iter().cloned())
            .collect()),
        ),
    ]);
    let d = DataSpec {
        json: INGEST_JSON,
        ts: Some("ts/client/src/service/origin/ingest.spec.ts"),
        rs: Some("rust/spec/src/asset/ingest.spec.rs"),
        ..DataSpec::DEFAULT
    };
    let mut outs = table_outputs(&root(), &d, table, &[])?;
    let (ts, rs) = (pack_ts(&pack)?, pack_rs(&pack)?);
    for o in &mut outs {
        let tail = if o.path.ends_with(".rs") { &rs } else { &ts };
        o.text = format!("{}\n{tail}", o.text);
    }
    Ok(outs)
}

fn pack_ts(pack: &Pack) -> Result<String, String> {
    let mut out = String::from(
        r#"/** The texture pack as one image, the file numbered `image`: `columns` tiles
 * of `cell` pixels per row, tile n at column n % columns and row n / columns,
 * `tiles` the number of the file each holds, and `top` the tile each cube
 * wears on top, by the cube's number. */
export const PACK = {
"#,
    );
    let _ = writeln!(out, "  image: {},", file_number(&pack.image)?);
    let _ = writeln!(out, "  cell: {},", pack.cell);
    let _ = writeln!(out, "  columns: {},", pack.columns);
    out.push_str("  tiles: [\n");
    for t in &pack.tiles {
        let _ = writeln!(out, "    {},", file_number(t)?);
    }
    out.push_str("  ],\n  top: {\n");
    for (group, ty, _, tile) in pack.faces.iter().filter(|f| f.2 == Face::Top.name()) {
        let _ = writeln!(out, "    {}: {tile},", cube_number(group, ty)?);
    }
    out.push_str("  } as Readonly<Record<number, number>>,\n} as const;\n");
    Ok(out)
}

fn pack_rs(pack: &Pack) -> Result<String, String> {
    let faces = members("face", &Face::ALL.map(Face::name))?;
    let mut out = vec![r##"use crate::asset::cube;
use crate::asset::id::AssetId;

/// Which of a box's images a tile is, as Minecraft's `cube_bottom_top`
/// names them. Which face of a box wears which is the renderer's
/// (paint.wgsl): a cell below the surface wears its bottom on every side.
{CUBE_FACE}

/// The texture pack as one image: `columns` tiles of `cell` pixels per row,
/// tile n at column n % columns and row n / columns, `tiles` the cube face
/// each tile is, and `sentinels` the tile a sentinel the world draws wears.
/// Its image is the file the catalog names for the cube kind's pack
/// (`cube::PACK`).
pub struct Pack {
    pub cell: u32,
    pub columns: u32,
    pub tiles: &'static [(AssetId, CubeFace, u32)],
    pub sentinels: &'static [(cube::Sentinel, u32)],
    /// What each tile reads as from far enough away that the tile itself is
    /// gone: its own mean color, taken in linear light. A mesh that
    /// summarizes a patch of ground takes its color from here rather than
    /// from the catalog's swatch, which is the color a type is filed under
    /// and not the color its material shows.
    pub means: &'static [[u8; 3]],
}
"##
    .replace(
        "{CUBE_FACE}",
        &rs_enum("CubeFace", &faces, "u8", "").join("\n"),
    )];
    out.extend([
        "pub const PACK: Pack = Pack {".to_string(),
        format!("    cell: {},", pack.cell),
        format!("    columns: {},", pack.columns),
        "    tiles: &[".to_string(),
    ]);
    for (group, ty, face, tile) in pack.faces.iter().filter(|f| f.0 != "sentinel") {
        let variant = if group == "object" {
            "Object"
        } else {
            "Ground"
        };
        let face = &faces
            .iter()
            .find(|m| m.name == *face)
            .ok_or(format!("{INGEST_JSON}: pack.faces: {face} is no face"))?
            .ident;
        out.push(format!(
            "        (cube::{variant}::{} as AssetId, CubeFace::{face}, {tile}),",
            cube_member(group, ty)?.ident
        ));
    }
    out.push("    ],".to_string());
    out.push("    sentinels: &[".to_string());
    out.extend(
        pack.faces
            .iter()
            .filter(|f| f.0 == "sentinel")
            .map(|(_, ty, _, tile)| {
                Ok(format!(
                    "        (cube::Sentinel::{}, {tile}),",
                    cube_member("sentinel", ty)?.ident
                ))
            })
            .collect::<Result<Vec<_>, String>>()?,
    );
    out.push("    ],".to_string());
    out.push("    means: &[".to_string());
    out.extend(
        pack.means
            .iter()
            .map(|m| format!("        [{}, {}, {}],", m[0], m[1], m[2])),
    );
    out.extend(["    ],".to_string(), "};".to_string()]);
    Ok(file(&out))
}
