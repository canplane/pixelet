// pxl-tool/spec/asset.rs

use xpute_gen::spec::data::enum_members;
use xpute_gen::spec::{rs_enum, ts_enum, Member};

use super::catalog::{slots, CUBE_JSON, DECOR_JSON};
use super::{file, json_str, line_comments, mark, read, rs_field, Node, Output};
const ID_JSON: &str = "spec/spec/asset/id.json5";
const COLOR_JSON: &str = "spec/spec/asset/color.json5";

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

/// An enum's members, `names` in their order, as the generator numbers and
/// names a list of them (`enum_members`).
pub(crate) fn members(name: &str, names: &[&str]) -> Result<Vec<Member>, String> {
    let names = Node::Arr(names.iter().map(|n| Node::Str(n.to_string())).collect());
    let list = Node::Obj(vec![
        ("$type".into(), Node::Str("enum".into())),
        ("$value".into(), names),
    ]);
    enum_members(name, &list)
}

/// A cube's number by its group (`sentinel`, `ground`, `object`) and name.
pub(crate) fn cube_number(group: &str, name: &str) -> Result<u32, String> {
    let m = cube_member(group, name)?;
    m.number.parse().map_err(|_| format!("{CUBE_JSON}: {} has no number", m.name))
}

/// The path the cube kind's texture pack ships at (`cube.json5` `pack`).
pub fn pack_path() -> Result<String, String> {
    let spec = read(CUBE_JSON)?;
    spec.get("pack")
        .map(Node::val)
        .and_then(Node::str)
        .map(str::to_string)
        .ok_or(format!("{CUBE_JSON}: no pack"))
}

/// The catalog's groups a texture is filed under: the ground's cubes, the
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

/// Each ground cube's color by name, packed `0xRRGGBB`, as color.json5 gives it.
pub fn ground_colors() -> Result<std::collections::BTreeMap<String, u32>, String> {
    let spec = read(COLOR_JSON)?;
    let ground = spec.get("ground").ok_or("color.json5 has no ground")?;
    ground
        .entries()
        .iter()
        .map(|(n, e)| {
            Ok((
                n.clone(),
                e.get("rgb").and_then(super::u32_of).ok_or(format!("color.json5: ground.{n} has no rgb"))?,
            ))
        })
        .collect()
}

pub(crate) fn color_outputs() -> Result<Vec<Output>, String> {
    let spec = read(COLOR_JSON)?;
    let head: Vec<String> = [
        "//",
        format!("// {}", mark("spec/spec/asset/color.json5", "pxl-tool spec/asset.rs")).as_str(),
        "//",
        "// The color of every ground cube and every placed color: what a cube *is* to",
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
        "/** Each cube's color, `0xRRGGBB`, by its number. */".to_string(),
        "export const CUBE_RGB: Readonly<Record<number, number>> = {".to_string(),
    ]);
    for (group, colors) in spec.entries() {
        if group == "$description" {
            continue;
        }
        for (n, e) in colors.entries() {
            let rgb = e.get("rgb").and_then(super::u32_of).ok_or(format!("color.json5: {group}.{n} has no rgb"))?;
            ts.push(format!("  {}: {rgb},", cube_number(group, n)?));
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
            .ok_or(format!("color.json5 has no {group}"))?;
        rs.extend([
            String::new(),
            format!("/// `{group}`'s color for a value."),
            format!("pub const fn {group}(t: {width}) -> Option<Swatch> {{"),
            format!("    match {e}::of(t) {{"),
        ]);
        for (name, entry) in colors.entries() {
            if !names.contains(name) {
                return Err(format!(
                    "color.json5: {group}.{name} is nothing cube.json5 names"
                ));
            }
            rs.extend(line_comments(&entry.doc().unwrap_or_default(), "        "));
            let rgb = entry.get("rgb").and_then(super::u32_of).ok_or(format!("color.json5: {group}.{name} has no rgb"))?;
            rs.push(format!(
                "        Some({e}::{}) => Some(Swatch::new({rgb})),",
                cube_member(group, name)?.ident
            ));
        }
        if group == "ground" {
            if let Some(n) = names.iter().find(|n| colors.get(n).is_none()) {
                return Err(format!("color.json5: ground.{n} has no color"));
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

/// `IDX_BITS` and the number of kind `name` (id.json5).
fn kind_layout(name: &str) -> Result<(u32, u32), String> {
    let id = read(ID_JSON)?;
    Ok((
        id.get("IDX_BITS").and_then(super::u32_of).ok_or(format!("{ID_JSON}: no IDX_BITS"))?,
        id.get("KIND")
            .and_then(|k| k.get(name))
            .and_then(super::u32_of)
            .ok_or(format!("{ID_JSON}: no kind {name}"))?,
    ))
}

pub(crate) fn decor_outputs() -> Result<Vec<Output>, String> {
    let spec = read(DECOR_JSON)?;
    let doc = spec.doc().unwrap_or_default();
    let head: Vec<String> = [
        "//",
        format!("// {}", mark("spec/spec/asset/decor.json5", "pxl-tool spec/asset.rs")).as_str(),
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
    let (rows, table) = decor_table(&spec, &ground_asset_ids()?, &slots()?)?;

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
    rs.extend(
        [
            "",
            "use crate::asset::catalog::Slot;",
            "use crate::asset::id::{asset_id, AssetId, Kind};",
            "use crate::ground::GroundId;",
        ]
        .map(String::from),
    );
    rs.extend(rows);
    rs.extend(table);
    Ok(vec![
        Output::new("ts/client/src/ui/shared/asset/decor.spec.ts", file(&ts)),
        Output::new("rust/spec/src/asset/decor.spec.rs", file(&rs)),
    ])
}

/// The ground's asset ids by name and number, what a decor may stand on.
fn ground_asset_ids() -> Result<Vec<(String, u32)>, String> {
    let spec = read(CUBE_JSON)?;
    cubes(&spec, "ground")
}

/// The ground asset ids a decor's `on` holds a bit for, as many as pxl-spec ground.rs
/// `GroundId` takes, in `ON_WORDS` words. The tool links no workspace crate, so it holds the
/// count itself, and the generated `Row::on`, sized from `GroundId`, refuses a table of another.
const GROUND_ID_CNT: u32 = 1 << u8::BITS;
const ON_WORDS: usize = (GROUND_ID_CNT / u32::BITS) as usize;

/// The decors' table, in the catalog's order, which is the order their variants are numbered
/// in: the rows, every number the GPU reads too (how the ground generates each decor, its
/// ground asset ids, its boxes), and each decor's models over its row. A model or a skin is its
/// slot among `slots`, the catalog's numbering.
fn decor_table(
    spec: &Node,
    asset_ids: &[(String, u32)],
    slots: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    // How a decor faces, Orientation's members.
    let orients = members("orient", &["random", "along_transport"])?;
    let slot_of = |path: &str| {
        slots
            .iter()
            .position(|p| p == path)
            .ok_or(format!("{path} is no slot of the catalog"))
    };
    struct Row {
        name: String,
        doc: Vec<String>,
        seed: u32,
        chance: u32,
        orient: String,
        on: [u32; ON_WORDS],
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
        let seed = k
            .get("seed")
            .and_then(super::u32_of)
            .ok_or(at("seed"))?;
        let density = k
            .get("density")
            .map(Node::val)
            .and_then(Node::num)
            .filter(|d| *d > 0.0 && *d <= 1.0)
            .ok_or(at("density"))?;
        let orient = k
            .str_at("orient")
            .and_then(|o| orients.iter().find(|m| m.name == o))
            .ok_or(at("orient"))?;
        let mut on = [0u32; ON_WORDS];
        for l in k.get("on").and_then(Node::arr).ok_or(at("on"))? {
            let v = l
                .str()
                .and_then(|l| asset_ids.iter().find(|(g, _)| g == l))
                .map(|(_, v)| *v)
                .filter(|v| *v < GROUND_ID_CNT)
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
                    .map(Node::val)
                    .and_then(Node::arr)
                    .filter(|s| s.len() == 3)
                    .map(|s| s.iter().map(super::u32_of).collect::<Option<Vec<_>>>());
                match (v.str_at("model"), size.flatten()) {
                    (Some(model), Some(size)) if size.iter().all(|&n| n >= 1) =>
                    {
                        sizes.push(format!(
                            "[{}]",
                            size.iter()
                                .map(u32::to_string)
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
            doc: k.doc().unwrap_or_default(),
            seed,
            chance: libm::rint(density * 4_294_967_296.0).min(f64::from(u32::MAX)) as u32,
            orient: orient.ident.clone(),
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
        String::new(),
        "/// The words of a row's `on`, a bit for each ground asset id.".into(),
        "pub const ON_WORDS: usize = (GroundId::MAX as usize + 1) / u32::BITS as usize;".into(),
    ]);
    portable.extend(
        [
            "",
            "/// How a decor the ground generates faces: any way, or along the road or rail its",
            "/// cell lies on.",
        ]
        .map(String::from),
    );
    portable.extend(rs_enum("Orientation", &orients, "u32", ""));
    portable.extend(
        [
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
            "    /// The ground asset ids it may stand on, a bit each.",
            "    pub on: [u32; ON_WORDS],",
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
            format!("        seed: {:#010x},", r.seed),
            format!("        chance: {},", r.chance),
            format!("        orient: Orientation::{},", r.orient),
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
        "    pub model: Slot,",
        "    pub skin: Slot,",
        "}",
        "",
        "/// A decor: its id, which is its place in `DECORS` over the decor kind, its row",
        "/// (how the ground generates it and the boxes its variants claim, `Row`) and its",
        "/// variants' models, in the row's order.",
        "#[derive(Clone, Copy, Debug, PartialEq)]",
        "pub struct Decor {",
        "    pub id: AssetId,",
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
            format!("        id: asset_id(Kind::Decor, {i}),"),
            format!("        row: &ROWS[{i}],"),
            "        variants: &[".to_string(),
        ]);
        rs.extend(r.models.iter().map(|v| format!("            {v},")));
        rs.extend(["        ],".to_string(), "    },".to_string()]);
    }
    rs.push("];".into());
    Ok((portable, rs))
}
