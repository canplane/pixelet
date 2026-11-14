// pxl-tool/spec/asset.rs

use xpute_gen::spec::data::enum_members;
use xpute_gen::spec::{ts_enum, Member};

use super::catalog::{slots, CUBE_JSON, DECOR_JSON};
use super::{file, js_num, json_str, mark, read, rs_field, rs_member, Node, Output};
const ID_JSON: &str = "spec/spec/asset/id.json";
const COLOR_JSON: &str = "spec/spec/asset/color.json";

/// A group's cubes (`sentinel`, `ground`, `object`), each its name and its
/// number, as the generator numbers its enum.
fn cubes(spec: &Node, group: &str) -> Result<Vec<(String, u32)>, String> {
    enum_members(group, spec.get(group).ok_or(format!("{CUBE_JSON}: no {group}"))?)?
        .into_iter()
        .map(|m| {
            let number = m.number.parse().map_err(|_| format!("{CUBE_JSON}: {} has no number", m.name))?;
            Ok((m.name, number))
        })
        .collect()
}

/// A cube's member of its group's enum (`sentinel`, `ground`, `object`), by
/// its name.
pub(crate) fn cube_member(group: &str, name: &str) -> Result<Member, String> {
    let spec = read(CUBE_JSON)?;
    let list = spec.get(group).ok_or(format!("{CUBE_JSON}: no {group}"))?;
    enum_members(group, list)?
        .into_iter()
        .find(|m| m.name == name)
        .ok_or(format!("{CUBE_JSON}: {group}.{name} is no cube"))
}

/// A cube's number by its group (`sentinel`, `ground`, `object`) and name.
pub(crate) fn cube_number(group: &str, name: &str) -> Result<u32, String> {
    let m = cube_member(group, name)?;
    m.number.parse().map_err(|_| format!("{CUBE_JSON}: {} has no number", m.name))
}

/// The path the cube kind's texture pack ships at (`cube.json` `pack`).
pub fn pack_path() -> Result<String, String> {
    let spec = read(CUBE_JSON)?;
    spec.get("pack")
        .map(Node::val)
        .and_then(Node::str)
        .map(str::to_string)
        .ok_or(format!("{CUBE_JSON}: no pack"))
}

/// The catalog's groups a texture is filed under: the ground's palette entries, the
/// object's colors, and the sentinels, of which the world draws the bedrock.
pub fn catalog() -> Result<Vec<(String, Vec<String>)>, String> {
    let spec = read(CUBE_JSON)?;
    ["ground", "object", "sentinel"]
        .into_iter()
        .map(|group| {
            let names = cubes(&spec, group)?.into_iter().map(|(n, _)| n);
            Ok((group.to_string(), names.collect()))
        })
        .collect()
}

/// Each ground palette entry's color by name, packed `0xRRGGBB`, as color.json gives it.
pub fn ground_colors() -> Result<std::collections::BTreeMap<String, u32>, String> {
    let spec = read(COLOR_JSON)?;
    let ground = spec.get("ground").ok_or("color.json has no ground")?;
    ground
        .entries()
        .iter()
        .map(|(n, e)| {
            Ok((
                n.clone(),
                super::rgb(
                    e.str_at("hex")
                        .ok_or(format!("color.json: ground.{n} has no hex"))?,
                )?,
            ))
        })
        .collect()
}

pub(crate) fn color_outputs() -> Result<Vec<Output>, String> {
    let spec = read(COLOR_JSON)?;
    let head: Vec<String> = [
        "//",
        format!("// {}", mark("spec/spec/asset/color.json", "pxl-tool spec/asset.rs")).as_str(),
        "//",
        "// The color of every palette entry and every placed color: what a cube *is* to",
        "// the eye when no texture is drawn over it — everything drawn with the",
        "// pack off reads this, the coarse cells included, which summarize it",
        "// the way they summarize the tiles with the pack on",
        "// (pxl-render paint.wgsl). A texture pack decorates these; it",
        "// does not replace them.",
    ]
    .map(String::from)
    .to_vec();

    let mut ts = vec!["// @pxl/client/ui/shared/asset/color.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    ts.extend([
        String::new(),
        "/** Each cube's color as `#rrggbb`, by its number. */".to_string(),
        "export const CUBE_HEX: Readonly<Record<number, string>> = {".to_string(),
    ]);
    for (group, colors) in spec.entries() {
        if group == "$doc" {
            continue;
        }
        for (n, e) in colors.entries() {
            let hex = e
                .str_at("hex")
                .ok_or(format!("color.json: {group}.{n} has no hex"))?;
            ts.push(format!("  {}: {},", cube_number(group, n)?, json_str(hex)));
        }
    }
    ts.push("};".to_string());

    let types = read(CUBE_JSON)?;
    let mut rs = vec!["// pxl-spec/asset/color.spec.rs".to_string()];
    rs.extend(head);
    rs.extend(
        [
            "",
            "use super::cube::{Ground, Object, Sentinel};",
            "use super::Swatch;",
            "use crate::color::rgb;",
        ]
        .map(String::from),
    );
    let groups = [
        ("sentinel", "Sentinel", "u16"),
        ("ground", "Ground", "u8"),
        ("object", "Object", "u16"),
    ];
    for (group, e, width) in groups {
        let names: Vec<String> = cubes(&types, group)?.into_iter().map(|(n, _)| n).collect();
        let colors = spec
            .get(group)
            .ok_or(format!("color.json has no {group}"))?;
        rs.extend([
            String::new(),
            format!("/// `{group}`'s color for a value."),
            format!("pub const fn {group}(t: {width}) -> Option<Swatch> {{"),
            format!("    match {e}::of(t) {{"),
        ]);
        for (name, entry) in colors.entries() {
            if !names.contains(name) {
                return Err(format!(
                    "color.json: {group}.{name} is nothing cube.json names"
                ));
            }
            if let Some(doc) = entry.str_at("$doc") {
                rs.push(format!("        // {doc}"));
            }
            let hex = entry
                .str_at("hex")
                .ok_or(format!("color.json: {group}.{name} has no hex"))?;
            rs.push(format!(
                "        Some({e}::{}) => Some(const {{ Swatch::new(rgb({})) }}),",
                cube_member(group, name)?.ident,
                json_str(hex)
            ));
        }
        if group == "ground" {
            if let Some(n) = names.iter().find(|n| colors.get(n).is_none()) {
                return Err(format!("color.json: ground.{n} has no color"));
            }
            rs.push("        // A sentinel a baked cell can hold.".to_string());
            rs.push("        None => sentinel(t as u16),".to_string());
        } else {
            rs.push("        _ => None,".to_string());
        }
        rs.extend(["    }", "}"].map(String::from));
    }
    Ok(vec![
        Output::new("ts/client/src/ui/shared/asset/color.spec.ts", file(&ts)),
        Output::new("rust/spec/src/asset/color.spec.rs", file(&rs)),
    ])
}

/// `IDX_BITS` and the number of kind `name` (id.json).
fn kind_layout(name: &str) -> Result<(u32, u32), String> {
    let id = read(ID_JSON)?;
    let number = |n: Option<&Node>| n.and_then(Node::num).map(|n| n as u32);
    Ok((
        number(id.get("IDX_BITS").map(Node::val)).ok_or(format!("{ID_JSON}: no IDX_BITS"))?,
        number(id.get("KIND").and_then(|k| k.get(name)))
            .ok_or(format!("{ID_JSON}: no kind {name}"))?,
    ))
}

pub(crate) fn decor_outputs() -> Result<Vec<Output>, String> {
    let spec = read(DECOR_JSON)?;
    let doc = spec.lines_at("$doc").unwrap_or_default();
    let head: Vec<String> = [
        "//",
        format!("// {}", mark("spec/spec/asset/decor.json", "pxl-tool spec/asset.rs")).as_str(),
        "//",
        "// The decors, the cube catalog's sibling: drawn as a model, not a cell the rays meet.",
        "//",
    ]
    .map(String::from)
    .to_vec();
    let decors = spec
        .get("decors")
        .ok_or(format!("{DECOR_JSON}: no decors"))?
        .entries();
    let (idx_bits, decor_kind) = kind_layout("decor")?;
    // A decor's id is its place in the table, over the decor kind.
    let ids: Vec<(&str, u32)> = decors
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (k.as_str(), decor_kind << idx_bits | i as u32))
        .collect();
    let (rows, table) = decor_table(&spec, &ground_palette()?, &slots()?)?;

    let mut ts = vec!["// @pxl/client/ui/shared/asset/decor.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    ts.extend(doc.iter().map(|l| format!("// {l}")));
    let members: Vec<Member> = ids
        .iter()
        .map(|(k, id)| Member {
            ident: k.to_string(),
            name: k.to_string(),
            number: id.to_string(),
            doc: Vec::new(),
        })
        .collect();
    let mut labels = Vec::new();
    for ((k, v), (_, id)) in decors.iter().zip(&ids) {
        let label = v
            .str_at("label")
            .ok_or(format!("{DECOR_JSON}: {k} has no label"))?;
        labels.push(format!("  {id}: {},", json_str(label)));
    }
    ts.extend(ts_enum("Decor", &members));
    ts.extend([
        String::new(),
        "/** The decors' ids, in the table's order. */".to_string(),
        format!(
            "export const DECORS: readonly number[] = [{}];",
            ids.iter()
                .map(|(_, id)| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
        "/** Each decor's name, the one every locale shows: an item has one name. */".to_string(),
        "export const DECOR_LABEL: Readonly<Record<number, string>> = {".to_string(),
    ]);
    ts.extend(labels);
    ts.push("};".to_string());

    let mut rs = vec!["// pxl-spec/asset/decor.spec.rs".to_string()];
    rs.extend(head.iter().cloned());
    rs.extend(doc.iter().map(|l| format!("// {l}")));
    rs.push(
        "#![allow(clippy::absolute_paths, reason = \"generated from the spec\")]".to_string(),
    );
    rs.extend(rows);
    rs.extend(table);
    Ok(vec![
        Output::new("ts/client/src/ui/shared/asset/decor.spec.ts", file(&ts)),
        Output::new("rust/spec/src/asset/decor.spec.rs", file(&rs)),
    ])
}

/// The ground's palette entries by name and number, what a decor may stand on.
fn ground_palette() -> Result<Vec<(String, u32)>, String> {
    let spec = read(CUBE_JSON)?;
    cubes(&spec, "ground")
}

/// The decors' table, in the catalog's order, which is the order their variants are numbered
/// in: the rows, every number the GPU reads too (how the ground generates each decor, its
/// palette entries, its boxes), and each decor's models over its row. A model or a skin is its
/// slot among `slots`, the catalog's numbering.
fn decor_table(
    spec: &Node,
    asset_ids: &[(String, u32)],
    slots: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    let slot_of = |path: &str| {
        slots
            .iter()
            .position(|p| p == path)
            .ok_or(format!("{path} is no slot of the catalog"))
    };
    struct Row {
        name: String,
        doc: Vec<String>,
        seed: String,
        chance: u32,
        orient: String,
        on: [u32; 8],
        sizes: Vec<String>,
        models: Vec<String>,
    }
    let mut rows = Vec::new();
    for (name, k) in spec
        .get("decors")
        .ok_or(format!("{DECOR_JSON}: no decors"))?
        .entries()
    {
        let at = |field: &str| format!("{DECOR_JSON}: {name}.{field}");
        let seed = k.str_at("seed").ok_or(at("seed"))?;
        u32::from_str_radix(seed.trim_start_matches("0x"), 16)
            .map_err(|e| format!("{}: {e}", at("seed")))?;
        let density = k
            .get("density")
            .and_then(Node::num)
            .filter(|d| *d > 0.0 && *d <= 1.0)
            .ok_or(at("density"))?;
        let orient = k
            .str_at("orient")
            .filter(|o| matches!(*o, "random" | "along_transport"))
            .ok_or(at("orient"))?;
        let mut on = [0u32; 8];
        for l in k.get("on").and_then(Node::arr).ok_or(at("on"))? {
            let v = l
                .str()
                .and_then(|l| asset_ids.iter().find(|(g, _)| g == l))
                .map(|(_, v)| *v)
                .filter(|v| *v < 256)
                .ok_or(format!("{}: {l:?} is no ground asset_id", at("on")))?;
            on[(v >> 5) as usize] |= 1 << (v & 31);
        }
        let (mut sizes, mut models) = (Vec::new(), Vec::new());
        for pack in k.get("packs").and_then(Node::arr).ok_or(at("packs"))? {
            let skin = slot_of(pack.str_at("skin").ok_or(at("packs"))?)?;
            for v in pack
                .get("variants")
                .and_then(Node::arr)
                .ok_or(at("packs"))?
            {
                let size = v
                    .get("size")
                    .and_then(Node::arr)
                    .filter(|s| s.len() == 3)
                    .map(|s| s.iter().map(Node::num).collect::<Option<Vec<_>>>());
                match (v.str_at("model"), size.flatten()) {
                    (Some(model), Some(size))
                        if size
                            .iter()
                            .all(|&n| n >= 1.0 && n.fract() == 0.0 && n <= f64::from(u16::MAX)) =>
                    {
                        sizes.push(format!(
                            "[{}]",
                            size.iter()
                                .map(|&n| js_num(n))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        models.push(format!(
                            "Variant {{ model: {}, skin: {skin} }}",
                            slot_of(model)?
                        ));
                    }
                    _ => return Err(at("packs")),
                }
            }
        }
        if sizes.is_empty() {
            return Err(at("packs"));
        }
        rows.push(Row {
            name: name.clone(),
            doc: k.lines_at("$doc").unwrap_or_default(),
            seed: seed.to_string(),
            chance: libm::rint(density * 4_294_967_296.0).min(f64::from(u32::MAX)) as u32,
            orient: orient.to_string(),
            on,
            sizes,
            models,
        });
    }
    let variant_cap = rows.iter().map(|r| r.sizes.len()).max().unwrap_or(1);
    let mut portable: Vec<String> = vec![String::new()];
    portable.extend([
        "/// The most variants a decor has.".into(),
        format!("pub const VARIANT_CAP: usize = {variant_cap};"),
    ]);
    portable.extend(
        [
            "",
            "/// How a decor the ground generates faces: any way, or along the road or rail its",
            "/// cell lies on.",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
            "#[repr(u32)]",
            "pub enum Orientation {",
            "    Random,",
            "    AlongTransport,",
            "}",
            "",
            "/// A decor as the ground generates it, in 32-bit words alone, laid out as a",
            "/// GPU reads the table from a buffer of these words.",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
            "#[repr(C)]",
            "pub struct Row {",
            "    /// Its scatter: changing it reshuffles every one the ground generates.",
            "    pub seed: u32,",
            "    /// The chance a cell of its holds one, over 2^32: a cell's mean",
            "    /// (pxl-content scatter.rs `cnt`).",
            "    pub chance: u32,",
            "    pub orient: Orientation,",
            "    /// The palette entries it may stand on, a bit each.",
            "    pub on: [u32; 8],",
            "    pub variant_cnt: u32,",
            "    /// Each variant's box, in its decor's cells; none past `variant_cnt`.",
            "    pub sizes: [[u32; 3]; VARIANT_CAP],",
            "}",
            "",
            "#[rustfmt::skip]",
        ]
        .map(String::from),
    );
    portable.push(format!("pub const ROWS: [Row; {}] = [", rows.len()));
    for r in &rows {
        portable.push(format!("    // {}", r.name));
        let mut sizes = r.sizes.clone();
        sizes.resize(variant_cap, "[0, 0, 0]".into());
        portable.extend([
            "    Row {".to_string(),
            format!("        seed: {},", r.seed),
            format!("        chance: {},", r.chance),
            format!("        orient: Orientation::{},", rs_member(&r.orient)),
            format!(
                "        on: [{}],",
                r.on.map(|w| format!("{w:#010x}")).join(", ")
            ),
            format!("        variant_cnt: {},", r.models.len()),
            format!("        sizes: [{}],", sizes.join(", ")),
            "    },".to_string(),
        ]);
    }
    portable.push("];".into());

    let mut rs: Vec<String> = [
        "",
        "/// A model and its pack's colormap, each a slot the catalog fills.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct Variant {",
        "    pub model: crate::asset::catalog::Slot,",
        "    pub skin: crate::asset::catalog::Slot,",
        "}",
        "",
        "/// A decor: its id, which is its place in `DECORS` over the decor kind, its row",
        "/// (how the ground generates it and the boxes its variants claim, `Row`) and its",
        "/// variants' models, in the row's order.",
        "#[derive(Clone, Copy, Debug, PartialEq)]",
        "pub struct Decor {",
        "    pub id: crate::asset::id::AssetId,",
        "    pub row: &'static Row,",
        "    pub variants: &'static [Variant],",
        "}",
        "",
    ]
    .map(String::from)
    .to_vec();
    let meta = spec
        .get("meta")
        .ok_or(format!("{DECOR_JSON}: no meta"))?
        .entries();
    rs.extend([
        "/// What a decor thing carries beside where it stands.".to_string(),
        "#[derive(Clone, Copy, Debug, PartialEq)]".into(),
        "pub struct DecorMeta {".into(),
    ]);
    for (field, ty) in meta {
        let ty = ty
            .str()
            .filter(|t| *t == "u32")
            .ok_or(format!("{DECOR_JSON}: meta.{field} is no u32"))?;
        rs.push(format!("    pub {}: {ty},", rs_field(field)));
    }
    rs.extend(["}".to_string(), String::new()]);
    rs.push(format!("pub const DECORS: [Decor; {}] = [", rows.len()));
    for (i, r) in rows.iter().enumerate() {
        rs.extend(
            r.doc
                .iter()
                .map(|l| format!("    // {l}").trim_end().to_string()),
        );
        rs.extend([
            "    Decor {".to_string(),
            format!("        id: crate::asset::id::asset_id(crate::asset::id::Kind::Decor, {i}),"),
            format!("        row: &ROWS[{i}],"),
            "        variants: &[".to_string(),
        ]);
        rs.extend(r.models.iter().map(|v| format!("            {v},")));
        rs.extend(["        ],".to_string(), "    },".to_string()]);
    }
    rs.push("];".into());
    Ok((portable, rs))
}
